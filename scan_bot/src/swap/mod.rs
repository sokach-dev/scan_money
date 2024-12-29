use std::{str::FromStr, sync::Arc};

use anyhow::{anyhow, Result};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use tracing::{debug, info};

use crate::sol_client::{
    client::SolanaMonitor, ASSOCIATED_TOKEN_PROGRAM, PUMP_ACCOUNT, PUMP_BUY_METHOD,
    PUMP_FEE_RECIPIENT, PUMP_GLOBAL, PUMP_PROGRAM, PUMP_SELL_METHOD, RENT_PROGRAM,
};

pub struct Swap {
    pub keypair: Arc<Keypair>,
}

impl Swap {
    pub fn new(keypair: Arc<Keypair>) -> Self {
        Self { keypair }
    }

    pub async fn swap(
        &self,
        is_buy: bool,
        address: &str,
        token_amount: u64,       // token amount
        sol_amount: u64,         // sol amount
        buy_again: Option<bool>, // buy again
    ) -> Result<Vec<String>> {
        let owner = self.keypair.pubkey();
        let mint =
            Pubkey::from_str(address).map_err(|e| anyhow!("failed to parse mint pubkey: {}", e))?;

        let program_id = spl_token::ID;
        let mut pump_method = PUMP_SELL_METHOD;

        if is_buy {
            pump_method = PUMP_BUY_METHOD;
        }

        let buy_again = buy_again.unwrap_or(false);

        let client = SolanaMonitor::default_client().await;

        let bonding_curve = client.get_bonding_curve(address).await?;
        let associated_bonding_curve = get_associated_token_address(&bonding_curve, &mint);

        let mint_ata = get_associated_token_address(&owner, &mint);

        debug!(
            "get_cache_info: address: {}, program_id: {}, bonding_curve: {}, bonding_curve_associated: {}, mint_ata: {}",
            address, program_id, bonding_curve, associated_bonding_curve, mint_ata
        );

        let mut create_instruction = None;
        if is_buy {
            if is_buy && !buy_again {
                debug!("Creating associated token account for mint {}", mint);
                create_instruction = Some(create_associated_token_account(
                    &owner,
                    &owner,
                    &mint,
                    &program_id,
                ));
            }
        }

        let pump_program = Pubkey::from_str_const(PUMP_PROGRAM);
        let input_accounts = match is_buy {
            true => {
                // 通过滑点计算最大值
                let input_accounts = vec![
                    AccountMeta::new_readonly(Pubkey::from_str_const(PUMP_GLOBAL), false),
                    AccountMeta::new(Pubkey::from_str_const(PUMP_FEE_RECIPIENT), false),
                    AccountMeta::new_readonly(mint, false),
                    AccountMeta::new(bonding_curve, false),
                    AccountMeta::new(associated_bonding_curve, false),
                    AccountMeta::new(mint_ata, false),
                    AccountMeta::new(owner, true),
                    AccountMeta::new_readonly(system_program::id(), false),
                    AccountMeta::new_readonly(program_id, false),
                    AccountMeta::new_readonly(Pubkey::from_str_const(RENT_PROGRAM), false),
                    AccountMeta::new_readonly(Pubkey::from_str_const(PUMP_ACCOUNT), false),
                    AccountMeta::new_readonly(pump_program, false),
                ];

                input_accounts
            }
            false => {
                let input_accounts = vec![
                    AccountMeta::new_readonly(Pubkey::from_str_const(PUMP_GLOBAL), false),
                    AccountMeta::new(Pubkey::from_str_const(PUMP_FEE_RECIPIENT), false),
                    AccountMeta::new_readonly(mint, false),
                    AccountMeta::new(bonding_curve, false),
                    AccountMeta::new(associated_bonding_curve, false),
                    AccountMeta::new(mint_ata, false),
                    AccountMeta::new(owner, true),
                    AccountMeta::new_readonly(system_program::id(), false),
                    AccountMeta::new_readonly(
                        Pubkey::from_str_const(ASSOCIATED_TOKEN_PROGRAM),
                        false,
                    ),
                    AccountMeta::new_readonly(program_id, false),
                    AccountMeta::new_readonly(Pubkey::from_str_const(PUMP_ACCOUNT), false),
                    AccountMeta::new_readonly(pump_program, false),
                ];

                input_accounts
            }
        };

        info!(
            "swap: is_buy: {}, address: {}, token_amount: {}, sol_amount_threshold: {}",
            is_buy, address, token_amount, sol_amount
        );

        let build_swap_instruction = Instruction::new_with_bincode(
            pump_program,
            &(pump_method, token_amount, sol_amount),
            input_accounts,
        );

        let mut instructions = vec![];
        if let Some(create_instruction) = create_instruction {
            instructions.push(create_instruction);
        }
        if token_amount > 0 {
            instructions.push(build_swap_instruction);
        }
        if instructions.is_empty() {
            return Err(anyhow!("No instructions to execute"));
        }

        debug!("instructions: {:?}", instructions);

        // tx::new_signed_and_send().await
        client
            .new_signed_and_send(&self.keypair, instructions)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::sol_client::get_pda;

    use super::*;

    const ADDRESS: &str = "9RMWVWuUj3TuydqxWHCEuge4mgrK3m38TTXt9jHXpump";

    #[test]
    fn test_associate_token_address() {
        let mint = Pubkey::from_str_const(ADDRESS);
        if let Ok(bonding_curve) = get_pda(ADDRESS, PUMP_PROGRAM) {
            assert_eq!(
                bonding_curve.to_string(),
                "GdrruzVPenx9ggk9gJahDG6veCTz9ngdReuc9Yqmrb8C"
            );
            let bonding_curve_associated = get_associated_token_address(&bonding_curve, &mint);
            assert_eq!(
                bonding_curve_associated.to_string(),
                "Bjo9N84GPZjpWejNKNhEabF1F74kceqWkWLNqWArzd1V"
            );
        }
    }
}
