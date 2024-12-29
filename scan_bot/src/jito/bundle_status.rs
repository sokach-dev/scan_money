use std::time::Duration;

use super::JITO;
use anyhow::{anyhow, Result};
use tokio::time::sleep;
use tracing::{error, info};

#[derive(Debug)]
struct BundleStatus {
    confirmation_status: Option<String>,
    err: Option<serde_json::Value>,
    transactions: Option<Vec<String>>,
}

impl JITO {
    pub async fn check_bundle_status(&self, bundle_uuid: &str) -> Result<()> {
        let max_retries = 10;
        let retry_delay = Duration::from_secs(2);

        for attempt in 1..=max_retries {
            info!(
                "Checking final bundle status (attempt {}/{})",
                attempt, max_retries
            );
            let status_response = self
                .client
                .get_in_flight_bundle_statuses(vec![bundle_uuid.to_string()])
                .await?;

            if let Some(result) = status_response.get("result") {
                if let Some(value) = result.get("value") {
                    if let Some(statuses) = value.as_array() {
                        if let Some(bundle_status) = statuses.get(0) {
                            if let Some(status) = bundle_status.get("status") {
                                match status.as_str() {
                                    Some("Landed") => {
                                        info!("Bundle landed on-chain. Checking final status...");
                                        return self.check_final_bundle_status(bundle_uuid).await;
                                    }
                                    Some("Pending") => {
                                        info!("Bundle is pending. Waiting...");
                                    }
                                    Some(status) => {
                                        info!("Unexpected bundle status: {}. Waiting...", status);
                                    }
                                    None => {
                                        info!("Unable to parse bundle status. Waiting...");
                                    }
                                }
                            } else {
                                info!("Status field not found in bundle status. Waiting...");
                            }
                        } else {
                            info!("Bundle status not found. Waiting...");
                        }
                    } else {
                        info!("Unexpected value format. Waiting...");
                    }
                } else {
                    info!("Value field not found in result. Waiting...");
                }
            } else if let Some(error) = status_response.get("error") {
                info!("Error checking bundle status: {:?}", error);
            } else {
                info!("Unexpected response format. Waiting...");
            }

            if attempt < max_retries {
                sleep(retry_delay).await;
            }
        }
        Ok(())
    }
    async fn check_final_bundle_status(&self, bundle_uuid: &str) -> Result<()> {
        let max_retries = 10;
        let retry_delay = Duration::from_secs(2);

        for attempt in 1..=max_retries {
            info!(
                "Checking final bundle status (attempt {}/{})",
                attempt, max_retries
            );

            let status_response = self
                .client
                .get_bundle_statuses(vec![bundle_uuid.to_string()])
                .await?;
            let bundle_status = get_bundle_status(&status_response)?;

            match bundle_status.confirmation_status.as_deref() {
                Some("confirmed") => {
                    info!("Bundle confirmed on-chain. Waiting for finalization...");
                    check_transaction_error(&bundle_status)?;
                }
                Some("finalized") => {
                    info!("Bundle finalized on-chain successfully!");
                    check_transaction_error(&bundle_status)?;
                    print_transaction_url(&bundle_status);
                    return Ok(());
                }
                Some(status) => {
                    info!(
                        "Unexpected final bundle status: {}. Continuing to poll...",
                        status
                    );
                }
                None => {
                    info!("Unable to parse final bundle status. Continuing to poll...");
                }
            }

            if attempt < max_retries {
                sleep(retry_delay).await;
            }
        }

        Err(anyhow!(
            "Failed to get finalized status after {} attempts",
            max_retries
        ))
    }
}

fn get_bundle_status(status_response: &serde_json::Value) -> Result<BundleStatus> {
    status_response
        .get("result")
        .and_then(|result| result.get("value"))
        .and_then(|value| value.as_array())
        .and_then(|statuses| statuses.get(0))
        .ok_or_else(|| anyhow!("Failed to parse bundle status"))
        .map(|bundle_status| BundleStatus {
            confirmation_status: bundle_status
                .get("confirmation_status")
                .and_then(|s| s.as_str())
                .map(String::from),
            err: bundle_status.get("err").cloned(),
            transactions: bundle_status
                .get("transactions")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
        })
}

fn check_transaction_error(bundle_status: &BundleStatus) -> Result<()> {
    if let Some(err) = &bundle_status.err {
        if err["Ok"].is_null() {
            info!("Transaction executed without errors.");
            Ok(())
        } else {
            error!("Transaction encountered an error: {:?}", err);
            Err(anyhow!("Transaction encountered an error"))
        }
    } else {
        Ok(())
    }
}

fn print_transaction_url(bundle_status: &BundleStatus) {
    if let Some(transactions) = &bundle_status.transactions {
        if let Some(tx_id) = transactions.first() {
            info!("Transaction URL: https://solscan.io/tx/{}", tx_id);
        } else {
            info!("Unable to extract transaction ID.");
        }
    } else {
        info!("No transactions found in the bundle status.");
    }
}
