use std::sync::OnceLock;

use core::Candle;
use flux_schema::{fb, WIRE_SCHEMA_VERSION};
use time::OffsetDateTime;
use zeromq::{Socket, SocketRecv, SocketSend};

pub const DEFAULT_LIVE_PUB: &str = "tcp://127.0.0.1:5556";
pub const DEFAULT_CHUNK_REP: &str = "tcp://127.0.0.1:5557";
pub const DEFAULT_SOURCE_ID: &str = "SIM";
pub const DEFAULT_INTERVAL: &str = "1s";
pub const DEFAULT_BACKFILL_LIMIT: u32 = 10_000;

#[derive(Debug, Clone)]
pub struct LiveConfig {
    pub live_pub: String,
    pub chunk_rep: String,
    pub source_id: String,
    pub interval: String,
}

impl Default for LiveConfig {
    fn default() -> Self {
        Self {
            live_pub: std::env::var("FLUX_LIVE_PUB").unwrap_or_else(|_| DEFAULT_LIVE_PUB.into()),
            chunk_rep: std::env::var("FLUX_CHUNK_REP").unwrap_or_else(|_| DEFAULT_CHUNK_REP.into()),
            source_id: std::env::var("FLUX_SOURCE_ID").unwrap_or_else(|_| DEFAULT_SOURCE_ID.into()),
            interval: std::env::var("FLUX_INTERVAL").unwrap_or_else(|_| DEFAULT_INTERVAL.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LiveEvent {
    CandleBatch {
        start_sequence: u64,
        candles: Vec<Candle>,
    },
    Error(String),
}

pub fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

pub async fn backfill_candles(
    cfg: &LiveConfig,
    symbol: &str,
    from_sequence_exclusive: Option<u64>,
    limit: u32,
) -> Result<(u64, Vec<Candle>), String> {
    let mut socket = zeromq::ReqSocket::new();
    socket
        .connect(&cfg.chunk_rep)
        .await
        .map_err(|e| format!("chunk_rep connect failed: {e}"))?;

    let req = encode_backfill_request(cfg, symbol, from_sequence_exclusive, limit);
    socket
        .send(req.into())
        .await
        .map_err(|e| format!("chunk_rep send failed: {e}"))?;

    let repl = socket
        .recv()
        .await
        .map_err(|e| format!("chunk_rep recv failed: {e}"))?;
    let bytes: Vec<u8> = repl
        .try_into()
        .map_err(|e| format!("chunk_rep response invalid: {e}"))?;
    decode_backfill_response(&bytes)
}

pub async fn subscribe_candles(
    cfg: LiveConfig,
    symbol: String,
    sender: tokio::sync::mpsc::UnboundedSender<LiveEvent>,
) -> Result<(), String> {
    let topic = topic_for(&cfg, &symbol);
    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&cfg.live_pub)
        .await
        .map_err(|e| format!("live_pub connect failed: {e}"))?;
    socket
        .subscribe(&topic)
        .await
        .map_err(|e| format!("subscribe failed: {e}"))?;

    loop {
        let msg = socket.recv().await.map_err(|e| format!("sub recv failed: {e}"))?;
        if msg.len() < 2 {
            continue;
        }
        let payload = msg.get(1).map(|b| b.as_ref()).unwrap_or(&[]);
        match decode_candle_batch(payload) {
            Ok((start_sequence, candles)) => {
                let _ = sender.send(LiveEvent::CandleBatch {
                    start_sequence,
                    candles,
                });
            }
            Err(err) => {
                let _ = sender.send(LiveEvent::Error(err));
            }
        }
    }
}

pub fn topic_for(cfg: &LiveConfig, symbol: &str) -> String {
    format!("candles.{}.{}.{}", cfg.source_id, symbol, cfg.interval)
}

fn encode_backfill_request(
    cfg: &LiveConfig,
    symbol: &str,
    from_sequence_exclusive: Option<u64>,
    limit: u32,
) -> Vec<u8> {
    let mut fbb = flatbuffers::FlatBufferBuilder::new();
    let key = build_stream_key(&mut fbb, cfg, symbol);
    let req = fb::BackfillCandlesRequest::create(
        &mut fbb,
        &fb::BackfillCandlesRequestArgs {
            key: Some(key),
            has_from_sequence: from_sequence_exclusive.is_some(),
            from_sequence_exclusive: from_sequence_exclusive.unwrap_or(0),
            has_end_ts_ms: false,
            end_ts_ms: 0,
            limit,
        },
    );
    let env = fb::Envelope::create(
        &mut fbb,
        &fb::EnvelopeArgs {
            schema_version: WIRE_SCHEMA_VERSION,
            type_hint: fb::MessageType::BACKFILL_CANDLES_REQUEST,
            correlation_id: None,
            message_type: fb::Message::BackfillCandlesRequest,
            message: Some(req.as_union_value()),
        },
    );
    fb::finish_envelope_buffer(&mut fbb, env);
    fbb.finished_data().to_vec()
}

fn decode_backfill_response(bytes: &[u8]) -> Result<(u64, Vec<Candle>), String> {
    let env = fb::root_as_envelope(bytes).map_err(|_| "invalid envelope".to_string())?;
    if env.schema_version() != WIRE_SCHEMA_VERSION {
        return Err("unsupported schema_version".to_string());
    }
    if env.message_type() == fb::Message::ErrorResponse {
        let msg = env
            .message_as_error_response()
            .and_then(|e| e.message().map(|s| s.to_string()))
            .unwrap_or_else(|| "error response".to_string());
        return Err(msg);
    }
    if env.message_type() != fb::Message::BackfillCandlesResponse {
        return Err(format!("unexpected message_type {:?}", env.message_type()));
    }
    let resp = env
        .message_as_backfill_candles_response()
        .ok_or_else(|| "missing BackfillCandlesResponse body".to_string())?;
    let candles = decode_candles(resp.candles()).map_err(|e| format!("decode candles: {e}"))?;
    Ok((resp.start_sequence(), candles))
}

fn decode_candle_batch(bytes: &[u8]) -> Result<(u64, Vec<Candle>), String> {
    let env = fb::root_as_envelope(bytes).map_err(|_| "invalid envelope".to_string())?;
    if env.schema_version() != WIRE_SCHEMA_VERSION {
        return Err("unsupported schema_version".to_string());
    }
    if env.message_type() != fb::Message::CandleBatch {
        return Err("not a candle batch".to_string());
    }
    let batch = env
        .message_as_candle_batch()
        .ok_or_else(|| "missing CandleBatch body".to_string())?;
    let candles = decode_candles(batch.candles()).map_err(|e| format!("decode candles: {e}"))?;
    Ok((batch.start_sequence(), candles))
}

fn decode_candles(
    candles: Option<flatbuffers::Vector<'_, flatbuffers::ForwardsUOffset<fb::Candle<'_>>>>,
) -> Result<Vec<Candle>, String> {
    let mut out = Vec::new();
    let Some(candles) = candles else {
        return Ok(out);
    };
    out.reserve(candles.len());
    for c in candles {
        let ts_ms = c.ts_ms();
        let timestamp = OffsetDateTime::from_unix_timestamp_nanos((ts_ms as i128) * 1_000_000)
            .map_err(|e| format!("invalid ts_ms={ts_ms}: {e}"))?;
        out.push(Candle {
            timestamp,
            open: c.open(),
            high: c.high(),
            low: c.low(),
            close: c.close(),
            volume: c.volume(),
        });
    }
    Ok(out)
}

fn build_stream_key<'a>(
    fbb: &mut flatbuffers::FlatBufferBuilder<'a>,
    cfg: &LiveConfig,
    symbol: &str,
) -> flatbuffers::WIPOffset<fb::StreamKey<'a>> {
    let source_id = fbb.create_string(&cfg.source_id);
    let symbol = fbb.create_string(symbol);
    let interval = fbb.create_string(&cfg.interval);
    fb::StreamKey::create(
        fbb,
        &fb::StreamKeyArgs {
            source_id: Some(source_id),
            symbol: Some(symbol),
            interval: Some(interval),
        },
    )
}

