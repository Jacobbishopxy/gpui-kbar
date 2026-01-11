use std::sync::OnceLock;

use core::Candle;
use flux_schema::{WIRE_SCHEMA_VERSION, fb};
use time::OffsetDateTime;
use tokio::time::sleep;
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
    let mut backoff_ms = 200u64;
    loop {
        match subscribe_candles_once(&cfg, &topic, &sender).await {
            Ok(()) => return Ok(()),
            Err(err) => {
                let _ = sender.send(LiveEvent::Error(err));
                sleep(std::time::Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms.saturating_mul(2)).min(5_000);
            }
        }
    }
}

pub fn topic_for(cfg: &LiveConfig, symbol: &str) -> String {
    format!("candles.{}.{}.{}", cfg.source_id, symbol, cfg.interval)
}

pub fn cursor_key_for(cfg: &LiveConfig, symbol: &str) -> String {
    format!("live_cursor.{}.{}.{}", cfg.source_id, symbol, cfg.interval)
}

async fn subscribe_candles_once(
    cfg: &LiveConfig,
    topic: &str,
    sender: &tokio::sync::mpsc::UnboundedSender<LiveEvent>,
) -> Result<(), String> {
    let mut socket = zeromq::SubSocket::new();
    socket
        .connect(&cfg.live_pub)
        .await
        .map_err(|e| format!("live_pub connect failed: {e}"))?;
    socket
        .subscribe(topic)
        .await
        .map_err(|e| format!("subscribe failed: {e}"))?;

    loop {
        let msg = socket
            .recv()
            .await
            .map_err(|e| format!("sub recv failed: {e}"))?;
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

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::path::PathBuf;

    use core::{DuckDbStore, StorageMode};
    use time::macros::datetime;
    use tokio::sync::oneshot;
    use tokio::time::timeout;
    use zeromq::{Socket, SocketRecv, SocketSend};

    use super::*;

    fn pick_unused_tcp_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("bind ephemeral port")
            .local_addr()
            .expect("local addr")
            .port()
    }

    fn temp_duckdb_path() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("gpui-kbar-live-roundtrip-{nonce}.duckdb"))
    }

    fn encode_candle_batch(
        cfg: &LiveConfig,
        symbol: &str,
        start_sequence: u64,
        candles: &[Candle],
    ) -> Vec<u8> {
        let mut fbb = flatbuffers::FlatBufferBuilder::new();
        let key = super::build_stream_key(&mut fbb, cfg, symbol);

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
        cfg: &LiveConfig,
        symbol: &str,
        start_sequence: u64,
        candles: &[Candle],
    ) -> Vec<u8> {
        let mut fbb = flatbuffers::FlatBufferBuilder::new();
        let key = super::build_stream_key(&mut fbb, cfg, symbol);

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
                has_more: false,
                next_sequence: 0,
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

    #[test]
    fn live_roundtrip_persists_and_restores_candles() {
        tokio_runtime().block_on(async {
            let live_port = pick_unused_tcp_port();
            let rep_port = pick_unused_tcp_port();

            let cfg = LiveConfig {
                live_pub: format!("tcp://127.0.0.1:{live_port}"),
                chunk_rep: format!("tcp://127.0.0.1:{rep_port}"),
                source_id: "SIM".to_string(),
                interval: "1s".to_string(),
            };
            let symbol = "TEST";

            let candles = vec![
                Candle {
                    timestamp: datetime!(2026-01-01 00:00:00 UTC),
                    open: 1.0,
                    high: 2.0,
                    low: 0.5,
                    close: 1.5,
                    volume: 10.0,
                },
                Candle {
                    timestamp: datetime!(2026-01-01 00:00:01 UTC),
                    open: 1.5,
                    high: 2.5,
                    low: 1.0,
                    close: 2.0,
                    volume: 11.0,
                },
                Candle {
                    timestamp: datetime!(2026-01-01 00:00:02 UTC),
                    open: 2.0,
                    high: 3.0,
                    low: 1.5,
                    close: 2.5,
                    volume: 12.0,
                },
                Candle {
                    timestamp: datetime!(2026-01-01 00:00:03 UTC),
                    open: 2.5,
                    high: 3.5,
                    low: 2.0,
                    close: 3.0,
                    volume: 13.0,
                },
                Candle {
                    timestamp: datetime!(2026-01-01 00:00:04 UTC),
                    open: 3.0,
                    high: 4.0,
                    low: 2.5,
                    close: 3.5,
                    volume: 14.0,
                },
            ];

            let (publish_tx, publish_rx) = oneshot::channel::<()>();

            let rep_addr = cfg.chunk_rep.clone();
            let rep_candles = candles.clone();
            let rep_cfg = cfg.clone();
            let rep_task = tokio::spawn(async move {
                let mut rep_socket = zeromq::RepSocket::new();
                rep_socket.bind(&rep_addr).await.expect("rep bind");
                loop {
                    let req = rep_socket.recv().await.expect("rep recv");
                    let req_bytes: Vec<u8> = req.try_into().expect("rep bytes");
                    let env = fb::root_as_envelope(&req_bytes).expect("valid envelope");
                    assert_eq!(env.schema_version(), WIRE_SCHEMA_VERSION);
                    assert_eq!(env.message_type(), fb::Message::BackfillCandlesRequest);
                    let req = env
                        .message_as_backfill_candles_request()
                        .expect("BackfillCandlesRequest body");
                    let from_exclusive = if req.has_from_sequence() {
                        req.from_sequence_exclusive()
                    } else {
                        0
                    };
                    let start_index = from_exclusive as usize;
                    let limit = req.limit().max(1) as usize;
                    let end_index = rep_candles.len().min(start_index.saturating_add(limit));
                    let slice = &rep_candles[start_index..end_index];
                    let resp = encode_backfill_response(
                        &rep_cfg,
                        symbol,
                        from_exclusive.saturating_add(1),
                        slice,
                    );
                    rep_socket.send(resp.into()).await.expect("rep send");
                }
            });

            let pub_addr = cfg.live_pub.clone();
            let pub_candles = candles.clone();
            let pub_cfg = cfg.clone();
            let pub_task = tokio::spawn(async move {
                let mut pub_socket = zeromq::PubSocket::new();
                pub_socket.bind(&pub_addr).await.expect("pub bind");
                let _ = publish_rx.await;
                let topic = topic_for(&pub_cfg, symbol);
                let payload = encode_candle_batch(&pub_cfg, symbol, 4, &pub_candles[3..]);
                let mut msg = zeromq::ZmqMessage::from(topic.as_str());
                msg.push_back(payload.into());
                pub_socket.send(msg).await.expect("pub send");
            });

            let path = temp_duckdb_path();
            let store = DuckDbStore::new(&path, StorageMode::Disk).expect("store");

            let (start, initial) = super::backfill_candles(&cfg, symbol, None, 3)
                .await
                .expect("backfill");
            assert_eq!(start, 1);
            assert_eq!(initial.len(), 3);
            store.write_candles(symbol, &initial).expect("write");
            store
                .set_session_value(&cursor_key_for(&cfg, symbol), "3")
                .expect("cursor");

            let topic = topic_for(&cfg, symbol);
            let mut sub = zeromq::SubSocket::new();
            sub.connect(&cfg.live_pub).await.expect("sub connect");
            sub.subscribe(&topic).await.expect("sub subscribe");

            let _ = publish_tx.send(());

            let msg = timeout(std::time::Duration::from_secs(2), sub.recv())
                .await
                .expect("recv timeout")
                .expect("recv ok");
            assert!(msg.len() >= 2);
            let payload = msg.get(1).unwrap().as_ref();
            let (start_sequence, live) = super::decode_candle_batch(payload).expect("decode batch");
            assert_eq!(start_sequence, 4);
            assert_eq!(live.len(), 2);
            store.append_candles(symbol, &live).expect("append");
            store
                .set_session_value(&cursor_key_for(&cfg, symbol), "5")
                .expect("cursor update");

            drop(store);
            let reopened = DuckDbStore::new(&path, StorageMode::Disk).expect("reopen store");
            let loaded = reopened.load_candles(symbol, None).expect("load");
            assert_eq!(loaded.len(), 5);
            assert_eq!(loaded.last().unwrap().close, 3.5);
            let cursor = reopened
                .get_session_value(&cursor_key_for(&cfg, symbol))
                .expect("cursor read")
                .expect("cursor exists");
            assert_eq!(cursor, "5");

            rep_task.abort();
            pub_task.abort();
        });
    }
}
