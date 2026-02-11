use crate::domain::entities::{Dex, PnL};
use crate::domain::events::{EventPublisher, ExecutionRequest, TradeFilled, TradeRejected};
use crate::infrastructure::{publish_event, TRADE_FILLED, TRADE_REJECTED};
use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_secs(1);

pub struct SolanaExecutor {
    publisher: Arc<dyn EventPublisher>,
    rpc_client: Arc<RpcClient>,
    keypair: Arc<Keypair>,
}

impl SolanaExecutor {
    pub fn new(
        publisher: Arc<dyn EventPublisher>,
        rpc_url: String,
        keypair_path: String,
    ) -> Result<Self> {
        let rpc_client = Arc::new(RpcClient::new_with_timeout_and_commitment(
            rpc_url,
            Duration::from_secs(10),
            CommitmentConfig::confirmed(),
        ));

        let keypair_json = std::fs::read_to_string(&keypair_path)?;
        let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_json)?;
        let keypair = Arc::new(Keypair::try_from(keypair_bytes.as_slice())?);

        Ok(Self {
            publisher,
            rpc_client,
            keypair,
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

        let signature = tokio::task::spawn_blocking(move || -> Result<String> {
            let recent_blockhash = rpc.get_latest_blockhash()?;

            let instructions = build_swap_instructions(entry_dex, &keypair.pubkey(), amount);

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

fn build_swap_instructions(dex: Dex, payer: &Pubkey, amount: f64) -> Vec<Instruction> {
    let mut instructions = vec![
        ComputeBudgetInstruction::set_compute_unit_limit(200_000),
        ComputeBudgetInstruction::set_compute_unit_price(1),
    ];

    let memo_data = match dex {
        Dex::Raydium => format!("raydium_swap:{}", amount),
        Dex::Orca => format!("orca_whirlpool_swap:{}", amount),
    };

    instructions.push(Instruction::new_with_bytes(
        Pubkey::new_unique(),
        memo_data.as_bytes(),
        vec![AccountMeta::new(*payer, true)],
    ));

    instructions
}
