use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;

pub const RAYDIUM_AMM_PROGRAM: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
pub const OPENBOOK_PROGRAM: &str = "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

pub struct RaydiumSwapAccounts {
    pub amm_id: Pubkey,
    pub amm_authority: Pubkey,
    pub open_orders: Pubkey,
    pub target_orders: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub serum_market: Pubkey,
    pub serum_bids: Pubkey,
    pub serum_asks: Pubkey,
    pub serum_event_queue: Pubkey,
    pub serum_coin_vault: Pubkey,
    pub serum_pc_vault: Pubkey,
    pub serum_vault_signer: Pubkey,
    pub user_source: Pubkey,
    pub user_dest: Pubkey,
    pub user_owner: Pubkey,
}

pub fn derive_amm_authority(nonce: u8) -> Result<Pubkey> {
    let program_id = Pubkey::from_str(RAYDIUM_AMM_PROGRAM)?;
    Ok(Pubkey::create_program_address(
        &[b"amm authority", &[nonce]],
        &program_id,
    )?)
}

pub fn build_raydium_swap_ix(
    accounts: &RaydiumSwapAccounts,
    amount_in: u64,
    minimum_amount_out: u64,
) -> Instruction {
    let program_id = Pubkey::from_str(RAYDIUM_AMM_PROGRAM).unwrap();
    let openbook = Pubkey::from_str(OPENBOOK_PROGRAM).unwrap();
    let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap();

    let mut data = Vec::with_capacity(17);
    data.push(9u8);
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&minimum_amount_out.to_le_bytes());

    let account_metas = vec![
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(accounts.amm_id, false),
        AccountMeta::new_readonly(accounts.amm_authority, false),
        AccountMeta::new(accounts.open_orders, false),
        AccountMeta::new(accounts.target_orders, false),
        AccountMeta::new(accounts.coin_vault, false),
        AccountMeta::new(accounts.pc_vault, false),
        AccountMeta::new_readonly(openbook, false),
        AccountMeta::new(accounts.serum_market, false),
        AccountMeta::new(accounts.serum_bids, false),
        AccountMeta::new(accounts.serum_asks, false),
        AccountMeta::new(accounts.serum_event_queue, false),
        AccountMeta::new(accounts.serum_coin_vault, false),
        AccountMeta::new(accounts.serum_pc_vault, false),
        AccountMeta::new_readonly(accounts.serum_vault_signer, false),
        AccountMeta::new(accounts.user_source, false),
        AccountMeta::new(accounts.user_dest, false),
        AccountMeta::new(accounts.user_owner, true),
    ];

    Instruction {
        program_id,
        accounts: account_metas,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_pubkey(n: u8) -> Pubkey {
        Pubkey::new_from_array([n; 32])
    }

    fn dummy_accounts() -> RaydiumSwapAccounts {
        RaydiumSwapAccounts {
            amm_id: dummy_pubkey(1),
            amm_authority: dummy_pubkey(2),
            open_orders: dummy_pubkey(3),
            target_orders: dummy_pubkey(4),
            coin_vault: dummy_pubkey(5),
            pc_vault: dummy_pubkey(6),
            serum_market: dummy_pubkey(7),
            serum_bids: dummy_pubkey(8),
            serum_asks: dummy_pubkey(9),
            serum_event_queue: dummy_pubkey(10),
            serum_coin_vault: dummy_pubkey(11),
            serum_pc_vault: dummy_pubkey(12),
            serum_vault_signer: dummy_pubkey(13),
            user_source: dummy_pubkey(14),
            user_dest: dummy_pubkey(15),
            user_owner: dummy_pubkey(16),
        }
    }

    #[test]
    fn test_raydium_swap_ix_discriminator_is_9() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        assert_eq!(ix.data[0], 9u8);
    }

    #[test]
    fn test_raydium_swap_ix_amount_in_encoding() {
        let amount_in: u64 = 1_500_000;
        let ix = build_raydium_swap_ix(&dummy_accounts(), amount_in, 0);
        assert_eq!(&ix.data[1..9], &amount_in.to_le_bytes());
    }

    #[test]
    fn test_raydium_swap_ix_min_out_encoding() {
        let min_out: u64 = 990_000;
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, min_out);
        assert_eq!(&ix.data[9..17], &min_out.to_le_bytes());
    }

    #[test]
    fn test_raydium_swap_ix_data_length_is_17() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        assert_eq!(ix.data.len(), 17);
    }

    #[test]
    fn test_raydium_swap_ix_has_18_accounts() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        assert_eq!(ix.accounts.len(), 18);
    }

    #[test]
    fn test_raydium_swap_ix_program_id() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        let expected = Pubkey::from_str(RAYDIUM_AMM_PROGRAM).unwrap();
        assert_eq!(ix.program_id, expected);
    }

    #[test]
    fn test_raydium_swap_ix_owner_is_signer() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        assert!(ix.accounts[17].is_signer);
    }

    #[test]
    fn test_raydium_swap_ix_owner_is_writable() {
        let ix = build_raydium_swap_ix(&dummy_accounts(), 1_000_000, 990_000);
        assert!(ix.accounts[17].is_writable);
    }

    #[test]
    fn test_derive_amm_authority_known_value() {
        let program_id = Pubkey::from_str(RAYDIUM_AMM_PROGRAM).unwrap();
        let (expected_authority, nonce) = Pubkey::find_program_address(&[b"amm authority"], &program_id);
        let derived = derive_amm_authority(nonce).unwrap();
        assert_eq!(derived, expected_authority);
        let known = Pubkey::from_str("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1").unwrap();
        assert_eq!(derived, known);
    }
}
