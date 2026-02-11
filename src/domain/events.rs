use crate::domain::entities::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, subject: &str, payload: &[u8]) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub pool_address: String,
    pub dex: Dex,
    pub base_asset: String,
    pub quote_asset: String,
    pub price: f64,
    pub liquidity: Liquidity,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpportunityDetected {
    pub opportunity: ArbitrageOpportunity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeIntent {
    pub opportunity_id: String,
    pub buy_dex: Dex,
    pub sell_dex: Dex,
    pub amount: f64,
    pub expected_profit: PnL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    pub trade_id: String,
    pub entry_dex: Dex,
    pub exit_dex: Dex,
    pub amount: f64,
    pub asset_pair: (String, String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeFilled {
    pub trade_id: String,
    pub entry_dex: Dex,
    pub exit_dex: Dex,
    pub amount: f64,
    pub asset_pair: (String, String),
    pub entry_price: f64,
    pub exit_price: f64,
    pub actual_profit: PnL,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRejected {
    pub trade_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionUpdate {
    pub position: Position,
}
