use anyhow::Result;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use std::str::FromStr;

pub const ORCA_WHIRLPOOL_PROGRAM: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const SWAP_DISCRIMINATOR: [u8; 8] = [248, 198, 158, 145, 225, 117, 135, 200];
pub const MIN_SQRT_PRICE: u128 = 4295048016;
pub const MAX_SQRT_PRICE: u128 = 79226673515401279992447579055;

pub struct OrcaSwapAccounts {
    pub whirlpool: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_vault_b: Pubkey,
    pub tick_array_0: Pubkey,
    pub tick_array_1: Pubkey,
    pub tick_array_2: Pubkey,
    pub oracle: Pubkey,
    pub user_token_a: Pubkey,
    pub user_token_b: Pubkey,
    pub user_authority: Pubkey,
}

pub fn derive_tick_arrays(
    whirlpool: &Pubkey,
    tick_current: i32,
    tick_spacing: u16,
    a_to_b: bool,
) -> Result<[Pubkey; 3]> {
    let program = Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM)?;
    let array_size = tick_spacing as i32 * 88;
    let offset = if a_to_b { -array_size } else { array_size };
    let start0 = tick_array_start_index(tick_current, tick_spacing);
    let start1 = start0 + offset;
    let start2 = start1 + offset;
    let derive = |start: i32| -> Result<Pubkey> {
        Ok(Pubkey::find_program_address(
            &[b"tick_array", whirlpool.as_ref(), &start.to_le_bytes()],
            &program,
        )
        .0)
    };
    Ok([derive(start0)?, derive(start1)?, derive(start2)?])
}

pub fn build_orca_swap_ix(
    accounts: &OrcaSwapAccounts,
    amount: u64,
    other_amount_threshold: u64,
    a_to_b: bool,
) -> Instruction {
    let program = Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM).unwrap();
    let token_program = Pubkey::from_str(TOKEN_PROGRAM_ID).unwrap();

    let sqrt_price_limit: u128 = if a_to_b { MIN_SQRT_PRICE } else { MAX_SQRT_PRICE };
    let mut data = Vec::with_capacity(42);
    data.extend_from_slice(&SWAP_DISCRIMINATOR);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&other_amount_threshold.to_le_bytes());
    data.extend_from_slice(&sqrt_price_limit.to_le_bytes());
    data.push(1u8);
    data.push(a_to_b as u8);

    let account_metas = vec![
        AccountMeta::new_readonly(token_program, false),
        AccountMeta::new(accounts.user_authority, true),
        AccountMeta::new(accounts.whirlpool, false),
        AccountMeta::new(accounts.user_token_a, false),
        AccountMeta::new(accounts.token_vault_a, false),
        AccountMeta::new(accounts.user_token_b, false),
        AccountMeta::new(accounts.token_vault_b, false),
        AccountMeta::new(accounts.tick_array_0, false),
        AccountMeta::new(accounts.tick_array_1, false),
        AccountMeta::new(accounts.tick_array_2, false),
        AccountMeta::new_readonly(accounts.oracle, false),
    ];

    Instruction {
        program_id: program,
        accounts: account_metas,
        data,
    }
}

fn tick_array_start_index(tick: i32, tick_spacing: u16) -> i32 {
    let array_size = tick_spacing as i32 * 88;
    if tick >= 0 {
        (tick / array_size) * array_size
    } else {
        ((tick - array_size + 1) / array_size) * array_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_pubkey(n: u8) -> Pubkey {
        Pubkey::new_from_array([n; 32])
    }

    fn dummy_accounts() -> OrcaSwapAccounts {
        OrcaSwapAccounts {
            whirlpool: dummy_pubkey(1),
            token_vault_a: dummy_pubkey(2),
            token_vault_b: dummy_pubkey(3),
            tick_array_0: dummy_pubkey(4),
            tick_array_1: dummy_pubkey(5),
            tick_array_2: dummy_pubkey(6),
            oracle: dummy_pubkey(7),
            user_token_a: dummy_pubkey(8),
            user_token_b: dummy_pubkey(9),
            user_authority: dummy_pubkey(10),
        }
    }

    #[test]
    fn test_orca_swap_ix_discriminator() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        assert_eq!(&ix.data[0..8], &SWAP_DISCRIMINATOR);
    }

    #[test]
    fn test_orca_swap_ix_amount_encoding() {
        let amount: u64 = 2_000_000;
        let ix = build_orca_swap_ix(&dummy_accounts(), amount, 0, true);
        assert_eq!(&ix.data[8..16], &amount.to_le_bytes());
    }

    #[test]
    fn test_orca_swap_ix_other_amount_encoding() {
        let threshold: u64 = 990_000;
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, threshold, true);
        assert_eq!(&ix.data[16..24], &threshold.to_le_bytes());
    }

    #[test]
    fn test_orca_swap_ix_data_length_is_42() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        assert_eq!(ix.data.len(), 42);
    }

    #[test]
    fn test_orca_swap_ix_has_11_accounts() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        assert_eq!(ix.accounts.len(), 11);
    }

    #[test]
    fn test_orca_swap_ix_program_id() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        let expected = Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM).unwrap();
        assert_eq!(ix.program_id, expected);
    }

    #[test]
    fn test_orca_swap_ix_a_to_b_true_uses_min_sqrt_price() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        let sqrt_price = u128::from_le_bytes(ix.data[24..40].try_into().unwrap());
        assert_eq!(sqrt_price, MIN_SQRT_PRICE);
    }

    #[test]
    fn test_orca_swap_ix_a_to_b_false_uses_max_sqrt_price() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, false);
        let sqrt_price = u128::from_le_bytes(ix.data[24..40].try_into().unwrap());
        assert_eq!(sqrt_price, MAX_SQRT_PRICE);
    }

    #[test]
    fn test_orca_swap_ix_amount_specified_is_input_true() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        assert_eq!(ix.data[40], 1u8);
    }

    #[test]
    fn test_orca_swap_ix_a_to_b_flag_in_data() {
        let ix_true = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        let ix_false = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, false);
        assert_eq!(ix_true.data[41], 1u8);
        assert_eq!(ix_false.data[41], 0u8);
    }

    #[test]
    fn test_orca_swap_ix_authority_is_signer() {
        let ix = build_orca_swap_ix(&dummy_accounts(), 1_000_000, 990_000, true);
        assert!(ix.accounts[1].is_signer);
    }

    #[test]
    fn test_tick_array_start_index_positive_tick() {
        assert_eq!(tick_array_start_index(1000, 64), 0);
    }

    #[test]
    fn test_tick_array_start_index_negative_tick() {
        assert_eq!(tick_array_start_index(-1, 64), -(64 * 88));
    }

    #[test]
    fn test_tick_array_start_index_at_exact_boundary() {
        let array_size = 64 * 88;
        assert_eq!(tick_array_start_index(array_size, 64), array_size);
    }
}
