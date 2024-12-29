use std::str::FromStr;

use anyhow::{anyhow, Result};
use sdk::JitoJsonRpcSDK;
use serde::Deserialize;
use serde_json::Value;
use solana_sdk::pubkey::Pubkey;
use tip_percentile::get_tip_percentile;
use tracing::error;

use crate::config::get_global_config;

pub mod bundle_status;
pub mod sdk;
pub mod tip_percentile;

#[derive(Debug, Clone, Deserialize)]
pub struct JITOConfig {
    pub tips_percentile: u32,
    pub tip_stream_url: String,
    pub jito_sdk_url: String, // https://mainnet.block-engine.jito.wtf/api/v1
    pub extra_tip: f64,       // 额外的小费 0.0001
    pub slippage: u64,        // 交易滑点 30表示30%
}

pub struct JITO {
    pub client: JitoJsonRpcSDK,
}

impl JITO {
    pub async fn default_client() -> Self {
        let c = get_global_config().await;
        let sdk = JitoJsonRpcSDK::new(&c.jito_config.jito_sdk_url, None);
        Self { client: sdk }
    }

    pub async fn get_random_tip_account(&self) -> Result<Pubkey> {
        let account = self.client.get_random_tip_account().await?;
        Ok(Pubkey::from_str(&account).inspect_err(|err| {
            error!("jito: failed to parse Pubkey: {:?}", err);
        })?)
    }
    pub async fn send_bundle(&self, bundle: Option<Value>, uuid: Option<&str>) -> Result<String> {
        let response = self.client.send_bundle(bundle, uuid).await?;
        let bundle_uuid = response["result"]
            .as_str()
            .ok_or_else(|| anyhow!("Failed to get bundle UUID from response"))?
            .to_string();
        Ok(bundle_uuid)
    }
}

// unit sol
pub async fn get_tip_value() -> Result<f64> {
    let c = get_global_config().await;
    let tip_percent = c.jito_config.tips_percentile;

    let tips = get_tip_percentile().await.read().await.clone();

    if let Some(ref data) = tips {
        match tip_percent {
            25 => Ok(data.landed_tips_25th_percentile),
            50 => Ok(data.landed_tips_50th_percentile),
            75 => Ok(data.landed_tips_75th_percentile),
            95 => Ok(data.landed_tips_95th_percentile),
            99 => Ok(data.landed_tips_99th_percentile),
            _ => Err(anyhow!("jito: invalid TIP_PERCENTILE value")),
        }
    } else {
        Err(anyhow!("jito: no tip percentile data available"))
    }
}
