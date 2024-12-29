use std::sync::Arc;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::config::get_global_config;

pub static TIP_PERCENTILE: OnceCell<Arc<RwLock<Option<TipPercentileData>>>> = OnceCell::const_new();
#[derive(Debug, Deserialize, Clone)]
pub struct TipPercentileData {
    pub time: String,
    pub landed_tips_25th_percentile: f64,
    pub landed_tips_50th_percentile: f64,
    pub landed_tips_75th_percentile: f64,
    pub landed_tips_95th_percentile: f64,
    pub landed_tips_99th_percentile: f64,
    pub ema_landed_tips_50th_percentile: f64,
}

pub async fn get_tip_percentile() -> &'static Arc<RwLock<Option<TipPercentileData>>> {
    TIP_PERCENTILE
        .get_or_init(|| async { Arc::new(RwLock::new(None)) })
        .await
}

pub async fn tip_stream() -> Result<()> {
    let c = get_global_config().await;
    let (ws_stream, _) = connect_async(&c.jito_config.tip_stream_url)
        .await
        .context("Failed to connect to WebSocket server")?;

    info!("Connected to WebSocket server: tip_stream");

    let (mut _write, mut read) = ws_stream.split();

    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => {
                debug!("Received text message: {}", text);

                match serde_json::from_str::<Vec<TipPercentileData>>(&text) {
                    Ok(data) => {
                        if !data.is_empty() {
                            let tp = get_tip_percentile().await;
                            tp.write().await.replace(data[0].clone());
                        } else {
                            warn!("Received an empty data.")
                        }
                    }
                    Err(e) => {
                        error!("Failed to deserialize JSON: {:?}", e);
                    }
                }
            }
            Ok(Message::Close(close)) => {
                info!("Connection closed: {:?}", close);
                break;
            }
            Err(e) => {
                error!("Error receiving message: {:?}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
