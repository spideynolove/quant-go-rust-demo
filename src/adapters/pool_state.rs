use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

pub fn fetch_raydium_nonce(rpc: &RpcClient, amm_id: &Pubkey) -> Result<u8> {
    let account = rpc.get_account(amm_id)?;
    extract_raydium_nonce(&account.data)
}

pub fn fetch_orca_tick_index(rpc: &RpcClient, whirlpool: &Pubkey) -> Result<(i32, u16)> {
    let account = rpc.get_account(whirlpool)?;
    extract_orca_state(&account.data)
}

fn extract_raydium_nonce(data: &[u8]) -> Result<u8> {
    if data.len() < 16 {
        return Err(anyhow!("raydium pool account too short: {} bytes", data.len()));
    }
    let nonce_u64 = u64::from_le_bytes(data[8..16].try_into()?);
    Ok(nonce_u64 as u8)
}

fn extract_orca_state(data: &[u8]) -> Result<(i32, u16)> {
    if data.len() < 85 {
        return Err(anyhow!("whirlpool account too short: {} bytes", data.len()));
    }
    let tick_spacing = u16::from_le_bytes(data[41..43].try_into()?);
    let tick_current = i32::from_le_bytes(data[81..85].try_into()?);
    Ok((tick_current, tick_spacing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raydium_nonce_extracted_from_byte_offset_8() {
        let mut fake_account = vec![0u8; 300];
        fake_account[8] = 254u8;
        let nonce = extract_raydium_nonce(&fake_account).unwrap();
        assert_eq!(nonce, 254u8);
    }

    #[test]
    fn test_orca_tick_index_extracted_from_correct_offset() {
        let mut fake_account = vec![0u8; 200];
        let tick: i32 = -12345;
        let tick_spacing: u16 = 64;
        fake_account[41..43].copy_from_slice(&tick_spacing.to_le_bytes());
        fake_account[81..85].copy_from_slice(&tick.to_le_bytes());
        let (extracted_tick, extracted_spacing) = extract_orca_state(&fake_account).unwrap();
        assert_eq!(extracted_tick, -12345i32);
        assert_eq!(extracted_spacing, 64u16);
    }

    #[test]
    fn test_orca_tick_index_account_too_short_returns_error() {
        let short_account = vec![0u8; 10];
        assert!(extract_orca_state(&short_account).is_err());
    }

    #[test]
    fn test_raydium_account_too_short_returns_error() {
        let short_account = vec![0u8; 5];
        assert!(extract_raydium_nonce(&short_account).is_err());
    }
}
