use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub nats: NatsConfig,
    pub solana: SolanaConfig,
    pub trading: TradingConfig,
    pub risk: RiskConfig,
    pub logging: LoggingConfig,
    pub dexs: DexsConfig,
    pub pools: PoolsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaConfig {
    pub rpc_url: String,
    pub ws_url: String,
    pub keypair_path: String,
    pub commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub min_spread_bps: u64,
    pub min_profit_usd: f64,
    pub max_position_size: f64,
    pub max_trade_size: f64,
    pub slippage_tolerance_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_open_positions: u64,
    pub max_daily_loss: f64,
    pub circuit_loss_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexsConfig {
    pub raydium: DexEndpointConfig,
    pub orca: DexEndpointConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexEndpointConfig {
    pub websocket_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolsConfig {
    pub raydium_sol_usdc: RaydiumPoolConfig,
    pub orca_sol_usdc: OrcaPoolConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaydiumPoolConfig {
    pub amm_id: String,
    pub open_orders: String,
    pub target_orders: String,
    pub coin_vault: String,
    pub pc_vault: String,
    pub coin_mint: String,
    pub pc_mint: String,
    pub serum_program: String,
    pub serum_market: String,
    pub serum_bids: String,
    pub serum_asks: String,
    pub serum_event_queue: String,
    pub serum_coin_vault: String,
    pub serum_pc_vault: String,
    pub serum_vault_signer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrcaPoolConfig {
    pub whirlpool: String,
    pub token_vault_a: String,
    pub token_vault_b: String,
    pub token_mint_a: String,
    pub token_mint_b: String,
    pub oracle: String,
    pub tick_spacing: u16,
}

pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let content = std::fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)?;
    Ok(config)
}

pub fn load_config_with_env<P: AsRef<Path>>(path: P) -> Result<Config> {
    let mut config = load_config(path)?;

    if let Ok(url) = std::env::var("NATS_URL") {
        config.nats.url = url;
    }
    if let Ok(url) = std::env::var("SOLANA_RPC_URL") {
        config.solana.rpc_url = url;
    }
    if let Ok(path) = std::env::var("SOLANA_KEYPAIR_PATH") {
        config.solana.keypair_path = path;
    }

    Ok(config)
}
