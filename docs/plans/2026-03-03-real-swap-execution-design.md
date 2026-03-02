# Real Swap Execution Design

**Date:** 2026-03-03
**Goal:** Replace placeholder swap instructions with real Raydium AMM v4 and Orca Whirlpool CPI calls, enabling both mainnet deployment and verifiable portfolio proof.

## Context

The existing system has correct architecture throughout — domain services, NATS event bus, WebSocket feeds, retry logic, position tracking. The single blocking gap is `build_swap_instructions` in `src/adapters/solana_executor.rs` (line 153), which uses `Pubkey::new_unique()` (random program ID) and writes a memo string instead of a real swap instruction. This would be rejected by the Solana runtime immediately.

Fixing this one function unblocks both goals simultaneously:
- **Portfolio**: real devnet simulations + mainnet tx signatures as evidence
- **Revenue**: bot can execute real arbitrage with $100–500 initial capital

## Scope

**In scope:**
- Real Raydium AMM v4 swap instruction (program `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`)
- Real Orca Whirlpool swap instruction (program `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`)
- Pool config for SOL/USDC on both devnet and mainnet
- Transaction simulation tests (zero-cost correctness proof)
- Criterion benchmarks for instruction build time and detection throughput
- README Live Results section

**Out of scope (Phase 2):**
- Jito bundle submission for MEV protection
- Additional trading pairs beyond SOL/USDC
- CLMM pools (Raydium v3)

## Architecture

No structural changes. Pool addresses are infrastructure config, not domain data — they do not flow through domain events.

```
SolanaExecutor
  └── pool_registry: HashMap<Dex, PoolConfig>   ← injected from config at startup
        ├── Dex::Raydium → RaydiumPoolConfig
        └── Dex::Orca    → OrcaPoolConfig
```

When an `ExecutionRequest` arrives, the executor looks up `entry_dex` in the registry, fetches the minimal runtime state (nonce for Raydium once at startup; tick index for Orca once per trade), then builds the real instruction.

## File Changes

| File | Type | Description |
|------|------|-------------|
| `Cargo.toml` | Modified | Add `spl-associated-token-account = "3.0"`, `bytemuck = { version = "1.0", features = ["derive"] }` |
| `src/infrastructure/config.rs` | Modified | Add `RaydiumPoolConfig`, `OrcaPoolConfig` structs |
| `config.toml` | Modified | Add `[pools.raydium_sol_usdc]` and `[pools.orca_sol_usdc]` sections |
| `src/adapters/pool_state.rs` | New | `fetch_raydium_nonce`, `fetch_orca_tick_index` |
| `src/adapters/raydium_swap.rs` | New | `build_raydium_swap_ix` |
| `src/adapters/orca_swap.rs` | New | `build_orca_swap_ix`, `derive_tick_arrays` |
| `src/adapters/solana_executor.rs` | Modified | Replace `build_swap_instructions` placeholder; inject pool registry |
| `src/adapters/mod.rs` | Modified | Export 3 new modules |
| `benches/execution.rs` | New | Criterion benchmarks |
| `README.md` | Modified | Add Live Results and Performance Benchmarks sections |

Domain events, domain services, NATS infrastructure, WebSocket feeds — unchanged.

## Raydium AMM v4 Instruction

**Program:** `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`

**Instruction data** (17 bytes, `SwapBaseIn` = discriminator `9`):
```
[9u8] ++ amount_in (u64 LE) ++ minimum_amount_out (u64 LE)
```

**Accounts** (18, in order):

| # | Account | Writable | Signer | Source |
|---|---------|----------|--------|--------|
| 0 | Token Program | - | - | constant |
| 1 | AMM ID | ✓ | - | config |
| 2 | AMM Authority (PDA) | - | - | derived: `create_program_address(&[amm_id.bytes(), &[nonce]], program_id)` |
| 3 | AMM Open Orders | ✓ | - | config |
| 4 | AMM Target Orders | ✓ | - | config |
| 5 | Pool Coin Vault | ✓ | - | config |
| 6 | Pool PC Vault | ✓ | - | config |
| 7 | Serum Program | - | - | constant |
| 8 | Serum Market | ✓ | - | config |
| 9 | Serum Bids | ✓ | - | config |
| 10 | Serum Asks | ✓ | - | config |
| 11 | Serum Event Queue | ✓ | - | config |
| 12 | Serum Coin Vault | ✓ | - | config |
| 13 | Serum PC Vault | ✓ | - | config |
| 14 | Serum Vault Signer | - | - | config |
| 15 | User Source ATA | ✓ | - | derived: `get_associated_token_address(owner, coin_mint)` |
| 16 | User Dest ATA | ✓ | - | derived: `get_associated_token_address(owner, pc_mint)` |
| 17 | User Owner | ✓ | ✓ | keypair |

**AMM authority derivation** (called once at startup, result cached):
```rust
Pubkey::create_program_address(
    &[&amm_id.to_bytes(), &[nonce]],
    &RAYDIUM_AMM_PROGRAM_ID,
)
```
`nonce` is read from bytes `[253]` of the pool account data (AMM v4 layout).

## Orca Whirlpool Instruction

**Program:** `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`

**Instruction discriminator** (sha256("global:swap")[0..8]):
```
[248, 198, 158, 145, 225, 117, 135, 200]
```

**Instruction data** (42 bytes):
```
discriminator (8) ++ amount (u64) ++ other_amount_threshold (u64)
++ sqrt_price_limit (u128) ++ amount_specified_is_input (bool) ++ a_to_b (bool)
```

**Accounts** (11, in order):

| # | Account | Writable | Signer | Source |
|---|---------|----------|--------|--------|
| 0 | Token Program | - | - | constant |
| 1 | User (authority) | - | ✓ | keypair |
| 2 | Whirlpool | ✓ | - | config |
| 3 | User Token A ATA | ✓ | - | derived |
| 4 | Token Vault A | ✓ | - | config |
| 5 | User Token B ATA | ✓ | - | derived |
| 6 | Token Vault B | ✓ | - | config |
| 7 | Tick Array 0 | ✓ | - | derived from tick_current_index |
| 8 | Tick Array 1 | ✓ | - | derived |
| 9 | Tick Array 2 | ✓ | - | derived |
| 10 | Oracle | - | - | config |

**Tick array derivation** (called once per Orca trade):
```rust
// 1. Fetch whirlpool account → tick_current_index, tick_spacing
// 2. Derive 3 consecutive tick array PDAs:
fn tick_array_start(tick: i32, spacing: u16, offset: i32) -> i32 {
    let array_size = spacing as i32 * 88;
    let start = (tick / array_size) * array_size;
    start + offset * array_size  // offset = 0, -1 or +1 depending on a_to_b
}
Pubkey::find_program_address(
    &[b"tick_array", whirlpool.as_ref(), &start.to_le_bytes()],
    &ORCA_WHIRLPOOL_PROGRAM_ID,
)
```

## RPC Call Budget

| Call | When | Count |
|------|------|-------|
| Fetch Raydium pool account (get nonce) | Startup | 1 total |
| Fetch Orca Whirlpool account (get tick index) | Per Orca trade | 1 per trade |
| `get_latest_blockhash` | Per transaction | 1 per tx (already exists) |
| `send_and_confirm_transaction` | Per transaction | 1 per tx (already exists) |

No other RPC calls. All pool account addresses are static config.

## Config Shape

```toml
[pools.raydium_sol_usdc]
amm_id = "..."
open_orders = "..."
target_orders = "..."
coin_vault = "..."
pc_vault = "..."
coin_mint = "So11111111111111111111111111111111111111112"
pc_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
serum_market = "..."
serum_bids = "..."
serum_asks = "..."
serum_event_queue = "..."
serum_coin_vault = "..."
serum_pc_vault = "..."
serum_vault_signer = "..."

[pools.orca_sol_usdc]
whirlpool = "..."
token_vault_a = "..."
token_vault_b = "..."
token_mint_a = "So11111111111111111111111111111111111111112"
token_mint_b = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
oracle = "..."
tick_spacing = 64
```

## Testing Strategy

### Phase 1 — Unit tests (no network)

Verify instruction byte layout correctness without any RPC calls:

```rust
test_raydium_swap_ix_byte_layout        // check discriminator, amount encoding, account count
test_raydium_amm_authority_derivation   // deterministic PDA math
test_orca_swap_ix_discriminator         // check first 8 bytes
test_orca_tick_array_derivation         // check PDA seeds and start index math
test_orca_swap_ix_account_count         // exactly 11 accounts
```

### Phase 2 — Simulation tests (devnet RPC, zero cost)

`RpcClient::simulate_transaction()` validates the full instruction against the live runtime without broadcasting or spending SOL. `SimulationResult { err: None }` proves the instruction is structurally accepted.

```rust
test_raydium_swap_simulation_passes     // devnet, simulate only
test_orca_swap_simulation_passes        // devnet, simulate only
```

Simulation results are logged with the RPC response for inclusion in README.

### Phase 3 — Mainnet live run

Start with $50 to prove execution. Target: first confirmed swap transaction signature on mainnet. Captures tx link for portfolio. Scales to $500 once execution is proven stable.

## Performance Benchmarks

Add `benches/execution.rs` (Criterion):

| Benchmark | Target |
|-----------|--------|
| `raydium_ix_build` | < 10 μs |
| `orca_ix_build` | < 10 μs |
| `opportunity_detection_throughput` | > 1,000 price updates/sec |

These fill the `⏳ Performance benchmarks` gap in the README.

## Portfolio Evidence

After Phase 2 and Phase 3, README gains:

```markdown
## Live Results

### Transaction Simulations (Devnet)
- Raydium SOL/USDC swap: simulation passed, 0 errors
- Orca SOL/USDC swap: simulation passed, 0 errors

### Mainnet Executions
| Date | Pool | Tx |
|------|------|----|
| 2026-03-XX | Raydium SOL/USDC | [solscan link] |
| 2026-03-XX | Orca SOL/USDC    | [solscan link] |

## Performance Benchmarks
| Metric | Result |
|--------|--------|
| Instruction build time | < 10 μs |
| Opportunity detection | > 1,000 updates/sec |
| Price-to-signal latency | < 1 ms |
```

This transforms the README from "demo" to "production evidence" — directly addressing the hiring bar in `docs/QuantJD.md`.
