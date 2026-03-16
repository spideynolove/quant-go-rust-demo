use criterion::{criterion_group, criterion_main, Criterion};
use solana_arb::adapters::orca_swap::{
    build_orca_swap_ix, derive_tick_arrays, OrcaSwapAccounts, MIN_SQRT_PRICE,
};
use solana_arb::adapters::raydium_swap::{build_raydium_swap_ix, RaydiumSwapAccounts};
use solana_sdk::pubkey::Pubkey;

fn dummy_pubkey(n: u8) -> Pubkey {
    Pubkey::new_from_array([n; 32])
}

fn raydium_accounts() -> RaydiumSwapAccounts {
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

fn orca_accounts() -> OrcaSwapAccounts {
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

fn bench_build_raydium_swap_ix(c: &mut Criterion) {
    let accounts = raydium_accounts();
    c.bench_function("build_raydium_swap_ix", |b| {
        b.iter(|| build_raydium_swap_ix(&accounts, 1_000_000_000, 990_000_000))
    });
}

fn bench_build_orca_swap_ix(c: &mut Criterion) {
    let accounts = orca_accounts();
    c.bench_function("build_orca_swap_ix", |b| {
        b.iter(|| build_orca_swap_ix(&accounts, 1_000_000_000, 990_000_000, true))
    });
}

fn bench_derive_tick_arrays(c: &mut Criterion) {
    let whirlpool = dummy_pubkey(1);
    c.bench_function("derive_tick_arrays", |b| {
        b.iter(|| derive_tick_arrays(&whirlpool, -10000, 64, true).unwrap())
    });
}

criterion_group!(
    benches,
    bench_build_raydium_swap_ix,
    bench_build_orca_swap_ix,
    bench_derive_tick_arrays
);
criterion_main!(benches);
