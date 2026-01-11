use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::Parser;
use flux_schema::{WIRE_SCHEMA_VERSION, fb};
use kbar_core::{Candle, LoadOptions, load_csv};
use time::format_description::well_known::Rfc3339;
use tokio::time::{Duration, interval};
use zeromq::{Socket, SocketRecv, SocketSend};

#[derive(Debug, Clone)]
struct StreamKeyOwned {
    source_id: String,
    symbol: String,
    interval: String,
}

impl StreamKeyOwned {
    fn topic(&self) -> String {
        format!(
            "candles.{}.{}.{}",
            self.source_id, self.symbol, self.interval
        )
    }
}

#[derive(Parser, Debug)]
#[command(name = "flux-mock-replay")]
#[command(about = "Mock Flux ZMQ PUB+REP service replaying CSV candles via FlatBuffers.", long_about = None)]
struct Args {
    #[arg(long, default_value = "tcp://0.0.0.0:5556")]
    live_pub: String,

    #[arg(long, default_value = "tcp://0.0.0.0:5557")]
    chunk_rep: String,

    #[arg(long, default_value = "SIM")]
    source_id: String,

    #[arg(long, default_value = "AAPL")]
    symbol: String,

    #[arg(long, default_value = "1s")]
    interval: String,

    #[arg(long, default_value = "data/candles/AAPL.csv")]
    csv: PathBuf,

    /// If non-zero, only read the first N candles from the CSV (faster startup).
    #[arg(long, default_value_t = 0)]
    limit_candles: usize,

    #[arg(long, default_value_t = 250)]
    tick_ms: u64,

    #[arg(long, default_value_t = 50)]
    batch_size: usize,

    #[arg(long, default_value_t = 1)]
    start_sequence: u64,
}

fn load_candles_for_mock(path: &PathBuf, limit: usize) -> Result<Vec<Candle>> {
    if limit == 0 {
        return load_csv(path, LoadOptions::default())
            .with_context(|| format!("load candles from {}", path.display()));
    }

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .with_context(|| format!("open csv {}", path.display()))?;
    let headers = reader.headers().context("read csv headers")?.clone();

    let idx = |name: &str| -> Result<usize> {
        headers
            .iter()
            .position(|h| h == name)
            .with_context(|| format!("missing column {name}"))
    };

    let ts_idx = idx("timestamp")?;
    let open_idx = idx("open")?;
    let high_idx = idx("high")?;
    let low_idx = idx("low")?;
    let close_idx = idx("close")?;
    let volume_idx = idx("volume")?;

    let mut out = Vec::with_capacity(limit);
    for record in reader.records().take(limit) {
        let record = record.context("read csv record")?;
        let ts = record.get(ts_idx).unwrap_or("");
        let timestamp = time::OffsetDateTime::parse(ts, &Rfc3339)
            .with_context(|| format!("parse timestamp {ts}"))?;
        out.push(Candle {
            timestamp,
            open: record
                .get(open_idx)
                .unwrap_or("")
                .parse()
                .context("parse open")?,
            high: record
                .get(high_idx)
                .unwrap_or("")
                .parse()
                .context("parse high")?,
            low: record
                .get(low_idx)
                .unwrap_or("")
                .parse()
                .context("parse low")?,
            close: record
                .get(close_idx)
                .unwrap_or("")
                .parse()
                .context("parse close")?,
            volume: record
                .get(volume_idx)
                .unwrap_or("")
                .parse()
                .context("parse volume")?,
        });
    }

    Ok(out)
}

fn build_stream_key<'a>(
    fbb: &mut flatbuffers::FlatBufferBuilder<'a>,
    key: &StreamKeyOwned,
) -> flatbuffers::WIPOffset<fb::StreamKey<'a>> {
    let source_id = fbb.create_string(&key.source_id);
    let symbol = fbb.create_string(&key.symbol);
    let interval = fbb.create_string(&key.interval);
    fb::StreamKey::create(
        fbb,
        &fb::StreamKeyArgs {
            source_id: Some(source_id),
            symbol: Some(symbol),
            interval: Some(interval),
        },
    )
}

fn encode_candle_batch(key: &StreamKeyOwned, start_sequence: u64, candles: &[Candle]) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key = build_stream_key(&mut fbb, key);

    let mut candle_offsets = Vec::with_capacity(candles.len());
    for candle in candles {
        let ts_ms = candle.timestamp.unix_timestamp_nanos() / 1_000_000;
        candle_offsets.push(fb::Candle::create(
            &mut fbb,
            &fb::CandleArgs {
                ts_ms: ts_ms as i64,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            },
        ));
    }
    let candle_vec = fbb.create_vector(&candle_offsets);
    let batch = fb::CandleBatch::create(
        &mut fbb,
        &fb::CandleBatchArgs {
            key: Some(key),
            start_sequence,
            candles: Some(candle_vec),
        },
    );

    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::CANDLE_BATCH,
            correlation_id: None,
            message_type: fb::Message::CandleBatch,
            message: Some(batch.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn encode_backfill_response(
    key: &StreamKeyOwned,
    start_sequence: u64,
    candles: &[Candle],
    has_more: bool,
    next_sequence: u64,
) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key = build_stream_key(&mut fbb, key);

    let mut candle_offsets = Vec::with_capacity(candles.len());
    for candle in candles {
        let ts_ms = candle.timestamp.unix_timestamp_nanos() / 1_000_000;
        candle_offsets.push(fb::Candle::create(
            &mut fbb,
            &fb::CandleArgs {
                ts_ms: ts_ms as i64,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            },
        ));
    }
    let candle_vec = fbb.create_vector(&candle_offsets);
    let resp = fb::BackfillCandlesResponse::create(
        &mut fbb,
        &fb::BackfillCandlesResponseArgs {
            key: Some(key),
            start_sequence,
            candles: Some(candle_vec),
            has_more,
            next_sequence,
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::BACKFILL_CANDLES_RESPONSE,
            correlation_id: None,
            message_type: fb::Message::BackfillCandlesResponse,
            message: Some(resp.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn encode_error(message: &str) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let msg = fbb.create_string(message);
    let err = fb::ErrorResponse::create(
        &mut fbb,
        &fb::ErrorResponseArgs {
            code: 1,
            message: Some(msg),
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::ERROR_RESPONSE,
            correlation_id: None,
            message_type: fb::Message::ErrorResponse,
            message: Some(err.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn main() -> Result<()> {
    let args = Args::parse();
    let candles = load_candles_for_mock(&args.csv, args.limit_candles)?;

    let key = StreamKeyOwned {
        source_id: args.source_id.clone(),
        symbol: args.symbol.clone(),
        interval: args.interval.clone(),
    };

    println!("topic: {}", key.topic());
    println!("live_pub bind: {}", args.live_pub);
    println!("chunk_rep bind: {}", args.chunk_rep);
    println!("csv: {}", args.csv.display());
    println!("candles: {}", candles.len());

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build tokio runtime")?;

    runtime.block_on(async move {
        let mut pub_socket = zeromq::PubSocket::new();
        pub_socket
            .bind(&args.live_pub)
            .await
            .with_context(|| format!("bind live_pub {}", args.live_pub))?;

        let mut rep_socket = zeromq::RepSocket::new();
        rep_socket
            .bind(&args.chunk_rep)
            .await
            .with_context(|| format!("bind chunk_rep {}", args.chunk_rep))?;

        let rpc_key = key.clone();
        let rpc_candles = candles.clone();
        let _rpc_task = tokio::spawn(async move {
            loop {
                let req = match rep_socket.recv().await {
                    Ok(req) => req,
                    Err(err) => {
                        eprintln!("rep recv error: {err}");
                        continue;
                    }
                };
                let req_bytes: Result<Vec<u8>, _> = req.try_into();
                let req_bytes = match req_bytes {
                    Ok(req_bytes) => req_bytes,
                    Err(err) => {
                        eprintln!("rep recv invalid message: {err}");
                        let _ = rep_socket
                            .send(encode_error("invalid request").into())
                            .await;
                        continue;
                    }
                };

                let resp = match fb::root_as_envelope(&req_bytes) {
                    Ok(env) => {
                        if env.schema_version() != WIRE_SCHEMA_VERSION {
                            encode_error("unsupported schema_version")
                        } else if env.message_type() != fb::Message::BackfillCandlesRequest {
                            encode_error("unsupported request message_type")
                        } else if let Some(req) = env.message_as_backfill_candles_request() {
                            if let Some(req_key) = req.key() {
                                let req_source = req_key.source_id().unwrap_or("");
                                let req_symbol = req_key.symbol().unwrap_or("");
                                let req_interval = req_key.interval().unwrap_or("");
                                if req_source != rpc_key.source_id
                                    || req_symbol != rpc_key.symbol
                                    || req_interval != rpc_key.interval
                                {
                                    encode_error("unknown stream key")
                                } else {
                                    let from_exclusive = if req.has_from_sequence() {
                                        req.from_sequence_exclusive()
                                    } else {
                                        0
                                    };
                                    let start_index = from_exclusive as usize;
                                    let limit = req.limit() as usize;
                                    let end_index = if limit == 0 {
                                        rpc_candles.len()
                                    } else {
                                        rpc_candles.len().min(start_index.saturating_add(limit))
                                    };
                                    let slice = if start_index < rpc_candles.len() {
                                        &rpc_candles[start_index..end_index]
                                    } else {
                                        &[]
                                    };
                                    let has_more = end_index < rpc_candles.len();
                                    let next_sequence = if has_more { end_index as u64 } else { 0 };
                                    encode_backfill_response(
                                        &rpc_key,
                                        from_exclusive.saturating_add(1),
                                        slice,
                                        has_more,
                                        next_sequence,
                                    )
                                }
                            } else {
                                encode_error("missing key")
                            }
                        } else {
                            encode_error("invalid BackfillCandlesRequest body")
                        }
                    }
                    Err(_) => encode_error("invalid envelope"),
                };

                if let Err(err) = rep_socket.send(resp.into()).await {
                    eprintln!("rep send error: {err}");
                }
            }
        });

        let topic = key.topic();
        let tick_duration = Duration::from_millis(args.tick_ms.max(1));
        let mut tick = interval(tick_duration);
        let mut index = args.start_sequence.saturating_sub(1) as usize;
        loop {
            tick.tick().await;
            if index >= candles.len() {
                index = 0;
            }

            let end = candles
                .len()
                .min(index.saturating_add(args.batch_size.max(1)));
            let slice = &candles[index..end];
            if slice.is_empty() {
                continue;
            }

            let start_sequence = index as u64 + 1;
            let payload = encode_candle_batch(&key, start_sequence, slice);

            let mut msg = zeromq::ZmqMessage::from(topic.as_str());
            msg.push_back(payload.into());
            if let Err(err) = pub_socket.send(msg).await {
                eprintln!("pub send error: {err}");
            }
            index = end;
        }

        #[allow(unreachable_code)]
        Ok(())
    })
}
