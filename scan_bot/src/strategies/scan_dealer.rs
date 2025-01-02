use crate::sol_client::TradeEvent;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::MonitorRule;
use anyhow::Result;
use chrono::Utc;
use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, info, warn};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ScanDealerConfig {
    #[validate(range(min = 0.0))]
    pub alarm_threshold: f64, // 警报阈值, 累计多少个sol
    pub check_interval: u64,         // 检查间隔(s)
    pub holding_time_threshold: u64, // 统计持仓时间阈值(s)
}

pub struct Statistics {
    // 在同一秒内，可能有多个事件，所以这里用Vec
    pub statistics_map: Arc<RwLock<HashMap<i64, HashMap<String, Vec<TradeEvent>>>>>, // (时间戳， (币， 交易量))
    pub holding_time_threshold: Duration, // 持仓时间阈值，超过这个时间就不跟踪了
    pub alarm_threshold: f64,             // 警报阈值，超过这个阈值就警报,累计多少个sol
}

static GLOBAL_STATISTICS_MANAGER: OnceCell<Arc<Statistics>> = OnceCell::const_new();

pub async fn init_statistics_manager() -> Result<()> {
    let c = crate::config::get_global_config().await;

    let statistics = Statistics {
        statistics_map: Arc::new(RwLock::new(HashMap::new())),
        holding_time_threshold: Duration::from_secs(c.scan_dealer_config.holding_time_threshold), // 1 mintue
        alarm_threshold: c.scan_dealer_config.alarm_threshold, // 10 sol
    };

    let statistics = Arc::new(statistics);

    GLOBAL_STATISTICS_MANAGER
        .set(statistics.clone())
        .map_err(|_| anyhow::anyhow!("Failed to set statistics manager"))?;

    Ok(())
}

pub async fn get_global_statistics_manager() -> &'static Arc<Statistics> {
    GLOBAL_STATISTICS_MANAGER
        .get()
        .expect("Statistics manager is not initialized")
}

impl Statistics {
    pub async fn start_monitor(self: Arc<Self>) {
        let statistics = self;
        info!("Start Statistics monitor");

        tokio::spawn(async move {
            let c = crate::config::get_global_config().await;
            let mut interval =
                tokio::time::interval(Duration::from_secs(c.scan_dealer_config.check_interval));
            loop {
                interval.tick().await;
                let mut remove_list = Vec::new();

                let ts_event_map = statistics.statistics_map.read().await.clone();
                debug!(
                    "Statistics monitor tick, amount_map: {:?}",
                    statistics.statistics_map
                );
                // alarm if needed
                for (ts, coins) in ts_event_map.iter() {
                    let record_ts = ts;
                    // 如果已经过去5s在来判断，否则放过
                    let now_ts = Utc::now().timestamp();
                    if (now_ts - record_ts) < 5 {
                        continue;
                    }
                    remove_list.push(*record_ts);
                    // 都是大于5s的数据
                    for (coin, events) in coins.iter() {
                        if events.len() < 3 {
                            continue; // 这个币记录太少
                        }
                        // 检查这些时间里的购买sol的数量是否都在15%的误差范围
                        let mut first_sol: f64 = 0.0;
                        let mut will_alarm = true;
                        for i in 0..3 {
                            let event = &events[i];
                            if i == 0 {
                                first_sol = event.data.sol_amount as f64 / 1_000_000_000.0;
                            } else {
                                let sol_amount = event.data.sol_amount as f64 / 1_000_000_000.0;
                                if (sol_amount - first_sol).abs() > first_sol * 0.15 {
                                    // 超过15%的误差
                                    will_alarm = false;
                                    break;
                                }
                            }
                        }
                        if will_alarm {
                            warn!("----> Alarm: {}, sol: {}", coin, first_sol);
                        }
                    }
                }

                {
                    let mut amount_map = statistics.statistics_map.write().await;
                    for ts in remove_list {
                        amount_map.remove(&ts);
                    }
                }
            }
        });
    }

    async fn add_event(&self, event: &TradeEvent) {
        // check shield
        debug!("Add event: {:?}", event);

        let sol_amount = event.data.sol_amount as f64 / 1_000_000_000.0;
        let price = event.data.get_price();
        if event.data.is_buy {
            if sol_amount < 0.5 {
                return;
            }
        }

        debug!(
            "Add event: {}, amount: {}, price: {}",
            event.data.mint, sol_amount, price
        );

        let mut event_map = self.statistics_map.write().await;
        let data = event.data.clone();
        event_map
            .entry(data.timestamp)
            .and_modify(|coins| {
                coins
                    .entry(data.mint.clone())
                    .and_modify(|events| {
                        events.push(event.clone());
                    })
                    .or_insert(vec![event.clone()]);
            })
            .or_insert({
                let mut map = HashMap::new();
                map.insert(data.mint.clone(), vec![event.clone()]);
                map
            });
    }
}

impl MonitorRule {
    pub async fn deal_scan_dealer(&self, event: &TradeEvent) -> Result<()> {
        let event = event.clone();
        tokio::spawn(async move {
            let statistics = get_global_statistics_manager().await.clone();
            statistics.add_event(&event).await;
        });
        Ok(())
    }
}
