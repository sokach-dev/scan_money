// use start_monitoring

use std::env;

use anyhow::Result;
use scan_bot::{
    sol_client::client::SolanaMonitor,
    strategies::{MonitorRule, MonitorRuleType},
};
use solana_client::rpc_response::RpcLogsResponse;
use tracing::info;
use utils::log::init_tracing;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    init_tracing();
    let mr = MonitorRule {
        address: "AqCx6U9vGzLC5pAA29pCNYyU3Qv6aTGUxoNCTMJnE68Z".to_string(), // pump init address
        rule_type: MonitorRuleType::ScanDealer,
    };
    let wss = env::var("WSS_SOLANA_URL")?;
    let rpc = env::var("RPC_SOLANA_URL")?;

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<RpcLogsResponse>(1000);

    // start monitoring in a new task
    tokio::spawn(async move {
        let sm = SolanaMonitor::new(&wss, &rpc);
        sm.start_log_subscribe(&mr.address, sender, None)
            .await
            .unwrap();
    });

    // receive logs
    while let Some(log) = receiver.recv().await {
        info!("Received log: {:?}", log);
    }
    Ok(())
}
