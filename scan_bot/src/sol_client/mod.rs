pub mod client;

use std::str::FromStr;

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as base64, Engine};
use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

pub const TEN_THOUSAND: u64 = 10000;
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const RENT_PROGRAM: &str = "SysvarRent111111111111111111111111111111111";
pub const ASSOCIATED_TOKEN_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";
pub const PUMP_GLOBAL: &str = "4wTV1YmiEkRvAtNtsSGPtUrqRYQMe5SKy2uB4Jjaxnjf";
pub const PUMP_FEE_RECIPIENT: &str = "CebN5WGQ4jvEPvsVU4EoHEpgzq1VV7AbicfhtW4xC9iM";
pub const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMP_ACCOUNT: &str = "Ce6TQqeHC9p8KetsN6JsjHK7UTZk7nasjjnr7XxXp9F1";
pub const PUMP_MINT: &str = "TSLvdd1pWpHVjahSpsvCXUbgwsL3JAcvokwaKt1eokM";
pub const PUMP_BUY_METHOD: u64 = 16927863322537952870;
pub const PUMP_SELL_METHOD: u64 = 12502976635542562355;

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct BondingCurveAccount {
    pub discriminator: u64,
    pub virtual_token_reserves: u64,
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub real_sol_reserves: u64,
    pub token_total_supply: u64,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEventData {
    pub mint: String,                // token mint address
    pub sol_amount: u64,             // buy or sell sol amount
    pub token_amount: u64,           // buy or sell token amount
    pub is_buy: bool,                // buy or sell
    pub user: String,                // user address
    pub timestamp: i64,              // timestamp
    pub virtual_sol_reserves: u64,   // virtual sol reserves
    pub virtual_token_reserves: u64, // virtual token reserves
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeEvent {
    pub name: String,
    pub data: TradeEventData,
}

impl TradeEventData {
    pub fn get_price(&self) -> f64 {
        let virtual_sol_reserves = self.virtual_sol_reserves as f64 / 1_000_000_000.0;
        let virtual_token_reserves = self.virtual_token_reserves as f64 / 1_000_000.0;
        virtual_sol_reserves / virtual_token_reserves
    }
}

pub fn parse_log_subscribe_data(program_data: &str) -> Result<TradeEvent> {
    // Remove "Program data: " prefix
    let data = program_data
        .strip_prefix("Program data: ")
        .ok_or_else(|| anyhow!("Invalid program data format"))?;

    // Decode base64
    let decoded = base64.decode(data)?;
    /* TradeEvent 和decoded的字节的关系如下：
       event_flag: 8 byte
       mint: 32 byte
       solAmount: 8 byte
       tokenAmount: 8 byte
       isBuy: 1 byte
       user: 32 byte
       timestamp: 8 byte
       virtualSolReserves: 8 byte
       virtualTokenReserves: 8 byte
       realSolReserves: 8 byte
       realTokenReserves: 8 byte
    */

    // 8 + 32 + 8 + 8 + 1 + 32 + 8 + 8 + 8 + 8 + 8 = 129
    if decoded.len() % 129 != 0 {
        return Err(anyhow!(format!(
            "Invalid program data length: {}",
            decoded.len()
        )));
    }
    // decode
    let mint_arr: [u8; 32] = decoded[8..40].try_into().unwrap();
    let user_arr: [u8; 32] = decoded[57..89].try_into().unwrap();
    let trade_event: TradeEvent = TradeEvent {
        name: "TradeEvent".to_string(),
        data: TradeEventData {
            mint: Pubkey::new_from_array(mint_arr).to_string(),
            sol_amount: u64::from_le_bytes(decoded[40..48].try_into().unwrap()),
            token_amount: u64::from_le_bytes(decoded[48..56].try_into().unwrap()),
            is_buy: decoded[56] != 0,
            user: Pubkey::new_from_array(user_arr).to_string(),
            timestamp: i64::from_le_bytes(decoded[89..97].try_into().unwrap()),
            virtual_sol_reserves: u64::from_le_bytes(decoded[97..105].try_into().unwrap()),
            virtual_token_reserves: u64::from_le_bytes(decoded[105..113].try_into().unwrap()),
            real_sol_reserves: u64::from_le_bytes(decoded[113..121].try_into().unwrap()),
            real_token_reserves: u64::from_le_bytes(decoded[121..129].try_into().unwrap()),
        },
    };

    Ok(trade_event)
}

impl BondingCurveAccount {
    pub fn get_price(&self) -> f64 {
        let virtual_sol_reserves = self.virtual_sol_reserves as f64 / 1_000_000_000.0;
        let virtual_token_reserves = self.virtual_token_reserves as f64 / 1_000_000.0;
        virtual_sol_reserves / virtual_token_reserves
    }
}

pub fn get_pda(mint: &str, program_id: &str) -> Result<Pubkey> {
    let mint = Pubkey::from_str(mint)?;
    let program_id = Pubkey::from_str(program_id)?;
    let seeds = [b"bonding-curve".as_ref(), mint.as_ref()];
    let (bonding_curve, _bump) = Pubkey::find_program_address(&seeds, &program_id);
    Ok(bonding_curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_program_data() -> Result<()> {
        /*
        {
            "name": "TradeEvent",
            "data": {
                "mint":"7R4zU5pgHFxRQaNUhhCAPFXaSN6AWiheD6rRfkFJpump",
                "solAmount": 1253951806,
                "tokenAmount": 37809162736217,
                "isBuy":  true,
                "user":  "ASxMiMb1AJGTU4AduPNB2CGqT1TiDqWkLvy7oCUnzw5x",
                "timestamp": 1734616564,
                "virtualSolReserves": 33306996548,
                "virtualTokenReserves": 966463606623031,
                "realSolReserves": 3306996548,
                "realTokenReserves": 686563606623031,
            }
        }
         */
        let program_data = "Program data: vdt/007mYe5fUJLKQBnZyU5a25rXFCHmUq3eDeg/6m3qXr6Y4LVhXz7JvUoAAAAAWdK2IWMiAAABjF9LiRHyIjjqqF93tZIAeB6MsYzDh6xG1Oi/PnwVBw/0JWRnAAAAAERvQMEHAAAANwv2V/5uAwBEwxzFAAAAADdz4wttcAIA";

        let event = parse_log_subscribe_data(program_data)?;

        println!("{:?}", event);
        assert_eq!(event.name, "TradeEvent");
        assert_eq!(
            event.data.mint,
            "7R4zU5pgHFxRQaNUhhCAPFXaSN6AWiheD6rRfkFJpump"
        );
        assert_eq!(event.data.sol_amount, 1253951806);
        assert_eq!(event.data.token_amount, 37809162736217);
        assert_eq!(event.data.is_buy, true);
        assert_eq!(
            event.data.user,
            "ASxMiMb1AJGTU4AduPNB2CGqT1TiDqWkLvy7oCUnzw5x"
        );
        assert_eq!(event.data.timestamp, 1734616564);
        assert_eq!(event.data.virtual_sol_reserves, 33306996548);
        assert_eq!(event.data.virtual_token_reserves, 966463606623031);
        assert_eq!(event.data.real_sol_reserves, 3306996548);
        assert_eq!(event.data.real_token_reserves, 686563606623031);

        Ok(())
    }
}
