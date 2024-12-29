use std::{env, sync::Arc};

use anyhow::Result;
use scan_bot::{jito::tip_percentile::tip_stream, swap::Swap};
use solana_sdk::signature::Keypair;
use tokio::time::Instant;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    utils::log::init_tracing();
    dotenv::dotenv()?;

    let private_key = env::var("PRIVATE_KEY")?;
    let keypair = Keypair::from_base58_string(&private_key);

    let coin_address = "4ToDu7A5PZ4n2q5CnKuNFY9rDd5rDEz1j3aHUGGvpump";

    let swap = Swap::new(Arc::new(keypair));

    // spawn a task to listen to the tip stream
    tokio::spawn(async move { tip_stream().await });

    let now = Instant::now();
    // buy
    if let Err(e) = swap
        .swap(true, coin_address, 100_000_000_000, 5_000_000, Some(true))
        .await
    {
        error!("Failed to swap: {}", e);
        return Err(e);
    }
    info!("buy ok Time elapsed: {:?}", now.elapsed());

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // sell
    if let Err(e) = swap
        .swap(false, coin_address, 100_000_000_000, 1_000_000, None)
        .await
    {
        error!("Failed to swap: {}", e);
        return Err(e);
    }

    info!("sell Time elapsed: {:?}", now.elapsed());

    Ok(())
}
