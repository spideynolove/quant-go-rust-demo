use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub symbol: String,
    pub mint_address: String,
    pub decimals: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pool {
    pub address: String,
    pub dex: Dex,
    pub base_asset: Asset,
    pub quote_asset: Asset,
    pub liquidity: Liquidity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dex {
    Raydium,
    Orca,
}

impl std::fmt::Display for Dex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Dex::Raydium => write!(f, "Raydium"),
            Dex::Orca => write!(f, "Orca"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Price {
    pub value: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Liquidity {
    pub base_amount: f64,
    pub quote_amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spread {
    pub basis_points: u64,
    pub absolute: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub id: String,
    pub asset_pair: (String, String),
    pub buy_dex: Dex,
    pub sell_dex: Dex,
    pub buy_price: Price,
    pub sell_price: Price,
    pub spread: Spread,
    pub estimated_profit: PnL,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub opportunity_id: String,
    pub entry_dex: Dex,
    pub exit_dex: Dex,
    pub amount: f64,
    pub expected_profit: PnL,
    pub status: TradeStatus,
    pub timestamp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TradeStatus {
    Pending,
    Submitted,
    Filled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub asset: String,
    pub amount: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub unrealized_pnl: PnL,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PnL {
    pub realized: f64,
    pub fees_paid: f64,
    pub net: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum ArbError {
    #[error("insufficient profit: expected {expected:.2}, minimum {minimum:.2}")]
    InsufficientProfit { expected: f64, minimum: f64 },

    #[error("risk limit exceeded: {0}")]
    RiskLimitExceeded(String),

    #[error("slippage exceeded: {actual_bps} bps > {max_bps} bps")]
    SlippageExceeded { actual_bps: u64, max_bps: u64 },

    #[error("insufficient liquidity for trade size {trade_size:.2}")]
    InsufficientLiquidity { trade_size: f64 },

    #[error("stale price data: age {age_ms}ms > max {max_ms}ms")]
    StalePriceData { age_ms: i64, max_ms: i64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_creation() {
        let asset = Asset {
            symbol: "SOL".to_string(),
            mint_address: "So11111111111111111111111111111111111111112".to_string(),
            decimals: 9,
        };
        assert_eq!(asset.symbol, "SOL");
        assert_eq!(asset.decimals, 9);
        assert!(!asset.mint_address.is_empty());
    }

    #[test]
    fn test_dex_display() {
        assert_eq!(Dex::Raydium.to_string(), "Raydium");
        assert_eq!(Dex::Orca.to_string(), "Orca");
    }

    #[test]
    fn test_pnl_net_calculation() {
        let realized = 100.0;
        let fees = 5.0;
        let net = realized - fees;
        let pnl = PnL {
            realized,
            fees_paid: fees,
            net,
        };
        assert!((pnl.net - 95.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dex_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Dex::Raydium);
        set.insert(Dex::Orca);
        set.insert(Dex::Raydium);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_arb_error_display() {
        let err = ArbError::InsufficientProfit {
            expected: 5.0,
            minimum: 10.0,
        };
        assert!(err.to_string().contains("insufficient profit"));
    }
}
