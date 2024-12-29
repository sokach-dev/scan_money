// use start_monitoring

use std::env;

use anyhow::Result;
use scan_bot::{
    sol_client::client::SolanaMonitor,
    strategies::{MonitorRule, MonitorRuleType},
};
use solana_client::rpc_response::RpcKeyedAccount;
use tracing::info;
use utils::log::init_tracing;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv()?;
    init_tracing();
    let mr = MonitorRule {
        address: "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P".to_string(),
        rule_type: MonitorRuleType::ScanDealer,
    };
    let wss = env::var("WSS_SOLANA_URL")?;
    let rpc = env::var("RPC_SOLANA_URL")?;

    let (sender, mut receiver) = tokio::sync::mpsc::channel::<RpcKeyedAccount>(1000);

    // start monitoring in a new task
    tokio::spawn(async move {
        let sm = SolanaMonitor::new(&wss, &rpc);
        sm.start_program_subscribe(&mr.address, sender)
            .await
            .unwrap();
    });

    // receive logs
    while let Some(log) = receiver.recv().await {
        info!("Received log: {:?}", log);
    }
    Ok(())
}
