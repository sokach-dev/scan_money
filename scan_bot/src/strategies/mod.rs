use anyhow::Result;
use scan_dealer::{get_global_statistics_manager, init_statistics_manager};
use serde::Deserialize;
use solana_client::rpc_response::RpcLogsResponse;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, OnceCell, RwLock};
use tracing::{debug, error};

use crate::{
    config::get_global_config,
    sol_client::{client::SolanaMonitor, parse_log_subscribe_data},
};

pub mod scan_dealer;

pub struct Shield {
    pub total_deal_amount: Arc<RwLock<HashMap<String, i64>>>, // 买入的时间
    pub currency_buy_coin_amount: Arc<RwLock<HashMap<String, (f64, i64)>>>, // (price, time)
}

static SHIELD: OnceCell<Arc<Shield>> = OnceCell::const_new();

pub async fn get_global_shield() -> &'static Arc<Shield> {
    SHIELD
        .get_or_init(|| async {
            let shield = Shield {
                total_deal_amount: Arc::new(RwLock::new(HashMap::new())),
                currency_buy_coin_amount: Arc::new(RwLock::new(HashMap::new())),
            };

            Arc::new(shield)
        })
        .await
}

#[derive(Clone, Debug, Deserialize)]
pub struct MonitorRule {
    pub address: String,            // 监控地址
    pub rule_type: MonitorRuleType, // 监控规则类型
}

#[derive(Clone, Debug, Deserialize)]
pub enum MonitorRuleType {
    ScanDealer, // 扫描庄
}

const DATA_FLAG: &str = "Program data: vdt";
const BUY_FLAG: &str = "Program log: Instruction: Buy";
const _SELL_FLAG: &str = "Program log: Instruction: Sell";
const _CREATE_FLAG: &str = "Program log: Instruction: Create";

impl MonitorRule {
    pub async fn should_alert(&self) -> Result<()> {
        let c = get_global_config().await;
        match self.rule_type {
            MonitorRuleType::ScanDealer => {
                // 监控快速上涨
                init_statistics_manager().await?;
                let statistic = get_global_statistics_manager().await.clone();
                statistic.start_monitor().await;

                // 监控pump_program,如果有币在短期内急速上涨,则买入
                let (sender, mut receiver) = mpsc::channel::<RpcLogsResponse>(1000);

                let address = self.address.clone();
                tokio::spawn(async move {
                    let solana_client = SolanaMonitor::new(&c.solana_wss_url, &c.solana_rpc_url);
                    solana_client
                        .start_log_subscribe(&address, sender, None)
                        .await
                        .unwrap();
                });

                while let Some(logs) = receiver.recv().await {
                    debug!("log: {:?}", logs);
                    if logs.logs.contains(&BUY_FLAG.to_string()) {
                        for log in logs.clone().logs {
                            if log.contains(&DATA_FLAG) {
                                if let Ok(event) = parse_log_subscribe_data(&log) {
                                    self.deal_scan_dealer(&event).await?;
                                } else {
                                    error!("parse_program_data error: {:?}", logs);
                                    break;
                                }
                                break;
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
