use crate::adapters::orca_swap::{
    build_orca_swap_ix, derive_tick_arrays, OrcaSwapAccounts, TOKEN_PROGRAM_ID as ORCA_TOKEN_PROGRAM,
};
use crate::adapters::pool_state::{fetch_orca_tick_index, fetch_raydium_nonce};
use crate::adapters::raydium_swap::{build_raydium_swap_ix, derive_amm_authority, RaydiumSwapAccounts};
use crate::domain::entities::{Dex, PnL};
use crate::domain::events::{EventPublisher, ExecutionRequest, TradeFilled, TradeRejected};
use crate::infrastructure::{publish_event, PoolsConfig, TRADE_FILLED, TRADE_REJECTED};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(1);
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1rb";
const SOL_DECIMALS: u64 = 1_000_000_000;
const SLIPPAGE_BPS: u64 = 50;

pub struct SolanaExecutor {
    publisher: Arc<dyn EventPublisher>,
    rpc_client: Arc<RpcClient>,
    keypair: Arc<Keypair>,
    pools: PoolsConfig,
    raydium_nonce: u8,
}

impl SolanaExecutor {
    pub fn new(
        publisher: Arc<dyn EventPublisher>,
        rpc_url: String,
        keypair_path: String,
        pools: PoolsConfig,
    ) -> Result<Self> {
        let rpc_client = Arc::new(RpcClient::new_with_timeout_and_commitment(
            rpc_url,
            Duration::from_secs(10),
            CommitmentConfig::confirmed(),
        ));

        let keypair_json = std::fs::read_to_string(&keypair_path)?;
        let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_json)?;
        let keypair = Arc::new(Keypair::try_from(keypair_bytes.as_slice())?);

        let amm_id = Pubkey::from_str(&pools.raydium_sol_usdc.amm_id)?;
        let raydium_nonce = fetch_raydium_nonce(&rpc_client, &amm_id)?;

        Ok(Self {
            publisher,
            rpc_client,
            keypair,
            pools,
            raydium_nonce,
        })
    }

    pub async fn execute_trade(&self, request: ExecutionRequest) -> Result<()> {
        info!(
            "Executing trade {} on {} -> {}",
            request.trade_id, request.entry_dex, request.exit_dex
        );

        match self.try_execute(&request).await {
            Ok(signature) => {
                let filled = TradeFilled {
                    trade_id: request.trade_id.clone(),
                    entry_dex: request.entry_dex,
                    exit_dex: request.exit_dex,
                    amount: request.amount,
                    asset_pair: request.asset_pair,
                    entry_price: 0.0,
                    exit_price: 0.0,
                    actual_profit: PnL {
                        realized: 0.0,
                        fees_paid: 0.000005,
                        net: -0.000005,
                    },
                };

                let payload = publish_event(&filled)?;
                self.publisher.publish(TRADE_FILLED, &payload).await?;

                info!("Trade {} filled: {}", request.trade_id, signature);
            }
            Err(e) => {
                let rejected = TradeRejected {
                    trade_id: request.trade_id.clone(),
                    reason: e.to_string(),
                };

                let payload = publish_event(&rejected)?;
                self.publisher.publish(TRADE_REJECTED, &payload).await?;

                error!("Trade {} rejected: {}", request.trade_id, e);
            }
        }

        Ok(())
    }

    async fn try_execute(&self, request: &ExecutionRequest) -> Result<String> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            match self.execute_single_attempt(request).await {
                Ok(signature) => return Ok(signature),
                Err(e) => {
                    warn!(
                        "Trade {} attempt {} failed: {}",
                        request.trade_id,
                        attempt + 1,
                        e
                    );
                    last_error = Some(e);

                    if attempt < MAX_RETRIES - 1 {
                        tokio::time::sleep(RETRY_DELAY).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Max retries exceeded")))
    }

    async fn execute_single_attempt(&self, request: &ExecutionRequest) -> Result<String> {
        let rpc = self.rpc_client.clone();
        let keypair = self.keypair.clone();
        let entry_dex = request.entry_dex;
        let amount = request.amount;
        let pools = self.pools.clone();
        let raydium_nonce = self.raydium_nonce;

        let signature = tokio::task::spawn_blocking(move || -> Result<String> {
            let recent_blockhash = rpc.get_latest_blockhash()?;

            let instructions = build_swap_instructions(
                entry_dex,
                &keypair.pubkey(),
                amount,
                &pools,
                raydium_nonce,
                &rpc,
            )?;

            let mut transaction =
                Transaction::new_with_payer(&instructions, Some(&keypair.pubkey()));
            transaction.sign(&[&keypair], recent_blockhash);

            let sig = rpc.send_and_confirm_transaction_with_spinner_and_config(
                &transaction,
                rpc.commitment(),
                RpcSendTransactionConfig {
                    skip_preflight: false,
                    preflight_commitment: Some(CommitmentConfig::confirmed().commitment),
                    ..Default::default()
                },
            )?;

            Ok(sig.to_string())
        })
        .await??;

        Ok(signature)
    }
}

fn derive_ata(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token_program = Pubkey::from_str(ORCA_TOKEN_PROGRAM).unwrap();
    let ata_program = Pubkey::from_str(ATA_PROGRAM).unwrap();
    Pubkey::find_program_address(
        &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ata_program,
    )
    .0
}

fn build_swap_instructions(
    dex: Dex,
    payer: &Pubkey,
    amount: f64,
    pools: &PoolsConfig,
    raydium_nonce: u8,
    rpc: &RpcClient,
) -> Result<Vec<Instruction>> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(200_000),
        ComputeBudgetInstruction::set_compute_unit_price(1),
    ];

    match dex {
        Dex::Raydium => {
            let p = &pools.raydium_sol_usdc;
            let coin_mint = Pubkey::from_str(&p.coin_mint)?;
            let pc_mint = Pubkey::from_str(&p.pc_mint)?;
            let amm_authority = derive_amm_authority(raydium_nonce)?;
            let accounts = RaydiumSwapAccounts {
                amm_id: Pubkey::from_str(&p.amm_id)?,
                amm_authority,
                open_orders: Pubkey::from_str(&p.open_orders)?,
                target_orders: Pubkey::from_str(&p.target_orders)?,
                coin_vault: Pubkey::from_str(&p.coin_vault)?,
                pc_vault: Pubkey::from_str(&p.pc_vault)?,
                serum_market: Pubkey::from_str(&p.serum_market)?,
                serum_bids: Pubkey::from_str(&p.serum_bids)?,
                serum_asks: Pubkey::from_str(&p.serum_asks)?,
                serum_event_queue: Pubkey::from_str(&p.serum_event_queue)?,
                serum_coin_vault: Pubkey::from_str(&p.serum_coin_vault)?,
                serum_pc_vault: Pubkey::from_str(&p.serum_pc_vault)?,
                serum_vault_signer: Pubkey::from_str(&p.serum_vault_signer)?,
                user_source: derive_ata(payer, &coin_mint),
                user_dest: derive_ata(payer, &pc_mint),
                user_owner: *payer,
            };
            let amount_in = (amount * SOL_DECIMALS as f64) as u64;
            let min_out =
                amount_in * (10_000 - SLIPPAGE_BPS) / 10_000;
            instructions.push(build_raydium_swap_ix(&accounts, amount_in, min_out));
        }
        Dex::Orca => {
            let p = &pools.orca_sol_usdc;
            let whirlpool = Pubkey::from_str(&p.whirlpool)?;
            let token_mint_a = Pubkey::from_str(&p.token_mint_a)?;
            let token_mint_b = Pubkey::from_str(&p.token_mint_b)?;
            let (tick_current, tick_spacing) = fetch_orca_tick_index(rpc, &whirlpool)?;
            let tick_arrays = derive_tick_arrays(&whirlpool, tick_current, tick_spacing, true)?;
            let accounts = OrcaSwapAccounts {
                whirlpool,
                token_vault_a: Pubkey::from_str(&p.token_vault_a)?,
                token_vault_b: Pubkey::from_str(&p.token_vault_b)?,
                tick_array_0: tick_arrays[0],
                tick_array_1: tick_arrays[1],
                tick_array_2: tick_arrays[2],
                oracle: Pubkey::from_str(&p.oracle)?,
                user_token_a: derive_ata(payer, &token_mint_a),
                user_token_b: derive_ata(payer, &token_mint_b),
                user_authority: *payer,
            };
            let amount_in = (amount * SOL_DECIMALS as f64) as u64;
            let min_out =
                (amount_in as u128 * (10_000 - SLIPPAGE_BPS) as u128 / 10_000) as u64;
            instructions.push(build_orca_swap_ix(&accounts, amount_in, min_out, true));
        }
    }

    Ok(instructions)
}
