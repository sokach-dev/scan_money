use anyhow::Result;
use tracing::{error, info};

use crate::{config::get_global_config, jito::tip_percentile::tip_stream};

pub async fn daemon() -> Result<()> {
    info!("daemon start");
    let c = get_global_config().await;
    for m in &c.monitors {
        info!("monitor: {:?}", m);

        // get tip stream
        info!("start tip stream");
        tokio::spawn(async move { tip_stream().await });

        // every monitor should have its own thread
        tokio::spawn(async move {
            if let Err(e) = m.should_alert().await {
                error!("Monitor error: {}, MonitorRult: {:?}", e, m);
            }
        });
    }
    // wait forever
    tokio::signal::ctrl_c().await?;

    Ok(())
}
