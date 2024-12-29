use anyhow::{anyhow, Result};
use borsh::from_slice;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{self},
    rpc_response::{Response, RpcKeyedAccount, RpcLogsResponse, RpcTokenAccountBalance},
};
use solana_sdk::{
    bs58,
    commitment_config::CommitmentConfig,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    system_instruction,
    transaction::Transaction,
};
use solana_transaction_status::{UiTransactionEncoding, UiTransactionStatusMeta};
use spl_token::ui_amount_to_amount;
use spl_token_client::{
    client::{ProgramClient, ProgramRpcClient, ProgramRpcClientSendTransaction},
    spl_token_2022::{extension::StateWithExtensionsOwned, state::Account},
    token::{TokenError, TokenResult},
};
use std::{env, str::FromStr, sync::Arc};
use tokio::{
    sync::{mpsc::Sender, RwLock},
    time::Instant,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use crate::{
    config::get_global_config,
    jito::{get_tip_value, JITO},
};

use super::{get_pda, BondingCurveAccount, PUMP_PROGRAM};

pub struct SolanaMonitor {
    websocket_url: String,
    rpc_client: RpcClient,
    noblocking_rpc_client: Arc<solana_client::nonblocking::rpc_client::RpcClient>,
}

impl SolanaMonitor {
    pub fn new(websocket_url: &str, rpc_url: &str) -> Self {
        Self {
            websocket_url: websocket_url.to_string(),
            rpc_client: RpcClient::new(rpc_url.to_string()),
            noblocking_rpc_client: Arc::new(
                solana_client::nonblocking::rpc_client::RpcClient::new(rpc_url.to_string()),
            ),
        }
    }

    pub async fn default_client() -> Self {
        let c = get_global_config().await;
        Self::new(&c.solana_wss_url, &c.solana_rpc_url)
    }

    pub async fn start_program_subscribe(
        &self,
        address: &str,
        sender: Sender<RpcKeyedAccount>,
    ) -> Result<()> {
        info!("Started monitoring program address: {}", address);
        let sub_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "programSubscribe",
            "params": [
                address,
                {
                    "commitment": "confirmed",
                    "encoding": "jsonParsed"
                }
            ]
        });
        let (ws_stream, _) = connect_async(&self.websocket_url).await?;
        let (mut write, mut read) = ws_stream.split();

        // subscribe
        write.send(Message::text(sub_msg.to_string())).await?;
        info!("Subscribe program subcribe successfully!");

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let v: Value = serde_json::from_str(&text)?;
                    if let Some(params) = v.get("params") {
                        if let Some(result) = params.get("result") {
                            if let Ok(info) =
                                serde_json::from_value::<Response<RpcKeyedAccount>>(result.clone())
                            {
                                if let Err(e) = sender.send(info.value.clone()).await {
                                    error!("Error sending message: {:?}", e);
                                } else {
                                    info!(
                                        "Send message: {:?}, capital: {}",
                                        info,
                                        sender.capacity()
                                    );
                                }
                            } else {
                                debug!("Receive can't parse json message: {:?}", result);
                            }
                        } else {
                            debug!("Receive not result message: {:?}", params);
                        }
                    } else {
                        debug!("Receive not params message: {:?}", v);
                    }
                }
                Ok(_) => {
                    info!("Receive not text message: {:?}", msg);
                }
                Err(e) => {
                    error!("Error receiving message: {:?}", e);
                }
            }
        }
        Ok(())
    }

    pub async fn start_log_subscribe(
        &self,
        address: &str,
        sender: Sender<RpcLogsResponse>,
        sub_success: Option<Arc<RwLock<bool>>>,
    ) -> Result<()> {
        // let (a, b) = PubsubClient::logs_subscribe(url, filter, config).await?;
        info!("Started monitoring log address: {}", address);
        // 实现订阅日志
        let sub_msg = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                {
                    "mentions": [address]
                },
                {
                    "commitment": "confirmed"
                }
            ]
        });
        let (ws_stream, _) = connect_async(&self.websocket_url).await?;
        let (mut write, mut read) = ws_stream.split();

        if sub_success.is_some() {
            *sub_success.unwrap().write().await = true;
        }

        //  subscribe
        write.send(Message::text(sub_msg.to_string())).await?;
        info!("Subscribe address: {} logs subcribe successfully!", address);

        while let Some(msg) = read.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    let v: Value = serde_json::from_str(&text)?;
                    if let Some(params) = v.get("params") {
                        if let Some(result) = params.get("result") {
                            if let Ok(log) =
                                serde_json::from_value::<Response<RpcLogsResponse>>(result.clone())
                            {
                                if log.value.err.is_none() {
                                    if let Err(e) = sender.send(log.value.clone()).await {
                                        error!("Error sending message: {:?}", e);
                                    } else {
                                        debug!(
                                            "Send message: {:?}, capital: {}",
                                            log.value,
                                            sender.capacity()
                                        );
                                    }
                                } else {
                                    debug!("Error receiving message: {:?}", result);
                                }
                            } else {
                                debug!("Receive can't parse json message: {:?}", result);
                            }
                        } else {
                            debug!("Receive not result message: {:?}", params);
                        }
                    } else {
                        debug!("Receive not params message: {:?}", v);
                    }
                }
                Ok(_) => {
                    info!("Receive not text message: {:?}", msg);
                }
                Err(e) => {
                    error!("Error receiving message: {:?}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn get_tx(&self, sig: &str) -> Result<UiTransactionStatusMeta> {
        // 实现获取交易信息
        let sig = Signature::from_str(sig)?;
        let tx = self.rpc_client.get_transaction_with_config(
            &sig,
            rpc_config::RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                commitment: Some(CommitmentConfig::confirmed()),
                max_supported_transaction_version: None,
            },
        )?;
        if let Some(meta) = tx.transaction.meta {
            if let Some(e) = meta.err {
                anyhow::bail!("Transaction error: {:?}", e);
            }
            return Ok(meta);
        }
        anyhow::bail!("Transaction not found")
    }

    pub async fn get_largest_accounts(&self, address: &str) -> Result<Vec<RpcTokenAccountBalance>> {
        let mint = Pubkey::from_str(address)?;
        let res = self
            .rpc_client
            .get_token_largest_accounts_with_commitment(&mint, CommitmentConfig::confirmed())?;

        Ok(res.value)
    }

    pub async fn get_bonding_curve_account(
        &self,
        address: &str,
    ) -> Result<(Pubkey, BondingCurveAccount)> {
        // 实现获取bonding curve账户信息
        let bonding_curve = self.get_bonding_curve(address).await?;
        // todo query associated bonding curve
        // get bonding curve data
        let bonding_curve_data = self
            .rpc_client
            .get_account_data(&bonding_curve)
            .inspect_err(|err| {
                warn!(
                    "Failed to get bonding curve account data address: {}, bonding curve: {}, err: {}",
                    address, bonding_curve, err
                );
            })?;

        let bonding_curve_account = from_slice::<BondingCurveAccount>(&bonding_curve_data)
            .map_err(|e| {
                anyhow!(
                    "Failed to decode bonding curve account data, err: {}",
                    e.to_string()
                )
            })?;
        Ok((bonding_curve, bonding_curve_account))
    }

    pub async fn get_bonding_curve(&self, address: &str) -> Result<Pubkey> {
        // 实现获取bonding curve账户信息
        let bonding_curve = get_pda(address, PUMP_PROGRAM)?;
        Ok(bonding_curve)
    }

    pub async fn get_account_info(
        &self,
        address: &Pubkey,
        account: &Pubkey,
    ) -> TokenResult<StateWithExtensionsOwned<Account>> {
        let program_client = Arc::new(ProgramRpcClient::new(
            self.noblocking_rpc_client.clone(),
            ProgramRpcClientSendTransaction,
        ));

        let account = program_client
            .get_account(*account)
            .await
            .map_err(TokenError::Client)?
            .ok_or(TokenError::AccountNotFound)
            .inspect_err(|err| {
                warn!(
                    "Failed to get account info, address: {}, err: {}",
                    address, err
                );
            })?;

        if account.owner != spl_token::ID {
            return Err(TokenError::AccountInvalidOwner);
        }
        let account = StateWithExtensionsOwned::<Account>::unpack(account.data)?;
        if account.base.mint != *address {
            return Err(TokenError::AccountInvalidMint);
        }

        Ok(account)
    }

    pub async fn new_signed_and_send(
        &self,
        keypair: &Keypair,
        mut instructions: Vec<Instruction>,
    ) -> Result<Vec<String>> {
        let start_time = Instant::now();
        let jito_client = JITO::default_client().await;
        let tip_account = jito_client.get_random_tip_account().await?;
        let c = get_global_config().await;
        let mut tip = get_tip_value().await?;
        tip = tip.min(0.2);
        tip += c.jito_config.extra_tip;
        let tip_lamports = ui_amount_to_amount(tip, spl_token::native_mint::DECIMALS);
        info!(
            "tip account: {}, extra tip:{} Tip: {} SOL, {} lamports",
            tip_account, c.jito_config.extra_tip, tip, tip_lamports
        );

        // send init tx
        instructions.push(system_instruction::transfer(
            &keypair.pubkey(),
            &tip_account,
            tip_lamports,
        ));
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;

        // let mut transaction = Transaction::new_with_payer(&instructions, Some(&keypair.pubkey()));
        let transaction = Transaction::new_signed_with_payer(
            &instructions,
            Some(&keypair.pubkey()),
            &vec![&*keypair],
            recent_blockhash,
        );

        // 使用SIMULATE可以查看构建的交易是否正确
        if env::var("TX_SIMULATE").ok() == Some("true".to_string()) {
            let result = self.rpc_client.simulate_transaction(&transaction.clone())?;
            if let Some(logs) = result.value.logs {
                for log in logs {
                    info!("Simulate log: {:?}", log);
                }
            }
            return Ok(vec![]);
        }

        let serialized_tx = bs58::encode(bincode::serialize(&transaction)?).into_string();
        let bundle = json!([serialized_tx]);

        let bundle_id = jito_client.send_bundle(Some(bundle), None).await?;

        debug!(
            "Send bundle id: {:?}, cost: {:?}",
            bundle_id,
            Instant::now().duration_since(start_time)
        );

        // todo check bundle status

        // jito_client.check_bundle_status(&bundle_id).await?;

        Ok(vec![])
    }
}
