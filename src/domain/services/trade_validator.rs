use crate::domain::entities::{ArbitrageOpportunity, PnL};
use crate::domain::events::{EventPublisher, OpportunityDetected, TradeIntent};
use crate::infrastructure::{publish_event, TRADE_INTENTS};
use std::sync::Arc;
use tracing::info;

pub struct TradeValidator {
    publisher: Arc<dyn EventPublisher>,
    config: ValidatorConfig,
}

pub struct ValidatorConfig {
    pub min_profit_usd: f64,
    pub max_position_size: f64,
    pub max_trade_size: f64,
    pub slippage_tolerance_bps: u64,
    pub fee_estimate_bps: u64,
    pub gas_cost_usd: f64,
    pub max_open_positions: u64,
    pub max_daily_loss: f64,
}

impl TradeValidator {
    pub fn new(publisher: Arc<dyn EventPublisher>, config: ValidatorConfig) -> Self {
        Self { publisher, config }
    }

    pub async fn validate_opportunity(&self, event: OpportunityDetected) -> anyhow::Result<()> {
        let opp = &event.opportunity;

        if !self.check_profitability(opp) {
            info!(
                "Opportunity {} rejected: insufficient profit after fees",
                opp.id
            );
            return Ok(());
        }

        if !self.check_risk_limits(opp) {
            info!("Opportunity {} rejected: risk limit exceeded", opp.id);
            return Ok(());
        }

        let trade_size = self.calculate_trade_size(opp);
        let estimated_profit = self.calculate_profit(opp, trade_size);

        info!(
            "Opportunity {} validated: size={:.2} expected profit ${:.2}",
            opp.id, trade_size, estimated_profit.net
        );

        let trade_intent = TradeIntent {
            opportunity_id: opp.id.clone(),
            buy_dex: opp.buy_dex,
            sell_dex: opp.sell_dex,
            amount: trade_size,
            expected_profit: estimated_profit,
        };

        let payload = publish_event(&trade_intent)?;
        self.publisher.publish(TRADE_INTENTS, &payload).await?;

        Ok(())
    }

    fn calculate_trade_size(&self, _opp: &ArbitrageOpportunity) -> f64 {
        self.config.max_trade_size.min(self.config.max_position_size)
    }

    fn check_profitability(&self, opp: &ArbitrageOpportunity) -> bool {
        let trade_size = self.calculate_trade_size(opp);
        let profit = self.calculate_profit(opp, trade_size);
        profit.net >= self.config.min_profit_usd
    }

    fn check_risk_limits(&self, opp: &ArbitrageOpportunity) -> bool {
        let trade_size = self.calculate_trade_size(opp);
        trade_size <= self.config.max_position_size
    }

    fn calculate_profit(&self, opp: &ArbitrageOpportunity, trade_size: f64) -> PnL {
        let units = trade_size / opp.buy_price.value;
        let gross_profit = opp.spread.absolute * units;
        let fees = trade_size * self.config.fee_estimate_bps as f64 / 10000.0;
        let slippage = trade_size * self.config.slippage_tolerance_bps as f64 / 10000.0;
        let total_costs = fees + slippage + self.config.gas_cost_usd;
        let net = gross_profit - total_costs;

        PnL {
            realized: gross_profit,
            fees_paid: total_costs,
            net,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::entities::{Dex, Price, Spread};

    fn create_test_opportunity(buy_price: f64, sell_price: f64) -> ArbitrageOpportunity {
        let absolute = sell_price - buy_price;
        let spread_bps = ((absolute / buy_price) * 10000.0) as u64;

        ArbitrageOpportunity {
            id: "test_opp".to_string(),
            asset_pair: ("SOL".to_string(), "USDC".to_string()),
            buy_dex: Dex::Raydium,
            sell_dex: Dex::Orca,
            buy_price: Price {
                value: buy_price,
                timestamp: 0,
            },
            sell_price: Price {
                value: sell_price,
                timestamp: 0,
            },
            spread: Spread {
                basis_points: spread_bps,
                absolute,
            },
            estimated_profit: PnL {
                realized: 0.0,
                fees_paid: 0.0,
                net: 0.0,
            },
            timestamp: 0,
        }
    }

    fn create_test_config() -> ValidatorConfig {
        ValidatorConfig {
            min_profit_usd: 0.1,
            max_position_size: 1000.0,
            max_trade_size: 100.0,
            slippage_tolerance_bps: 10,
            fee_estimate_bps: 50,
            gas_cost_usd: 0.001,
            max_open_positions: 5,
            max_daily_loss: 100.0,
        }
    }

    struct MockPublisher;

    #[async_trait::async_trait]
    impl EventPublisher for MockPublisher {
        async fn publish(&self, _subject: &str, _payload: &[u8]) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_profit_calculation_correctness() {
        let config = create_test_config();
        let publisher: Arc<dyn EventPublisher> = Arc::new(MockPublisher);
        let validator = TradeValidator::new(publisher, config);

        let opp = create_test_opportunity(100.0, 101.0);
        let trade_size = validator.calculate_trade_size(&opp);
        let profit = validator.calculate_profit(&opp, trade_size);

        let expected_units = 100.0 / 100.0;
        let expected_gross = 1.0 * expected_units;
        let expected_fees = 100.0 * 50.0 / 10000.0;
        let expected_slippage = 100.0 * 10.0 / 10000.0;
        let expected_net = expected_gross - expected_fees - expected_slippage - 0.001;

        assert!((profit.realized - expected_gross).abs() < 0.001);
        assert!((profit.net - expected_net).abs() < 0.001);
    }

    #[test]
    fn test_check_profitability_small_spread() {
        let config = ValidatorConfig {
            min_profit_usd: 10.0,
            max_position_size: 1000.0,
            max_trade_size: 100.0,
            slippage_tolerance_bps: 10,
            fee_estimate_bps: 50,
            gas_cost_usd: 0.001,
            max_open_positions: 5,
            max_daily_loss: 100.0,
        };
        let publisher: Arc<dyn EventPublisher> = Arc::new(MockPublisher);
        let validator = TradeValidator::new(publisher, config);

        let opp = create_test_opportunity(100.0, 100.01);
        assert!(!validator.check_profitability(&opp));
    }

    #[test]
    fn test_risk_limits_trade_size() {
        let config = ValidatorConfig {
            min_profit_usd: 0.1,
            max_position_size: 50.0,
            max_trade_size: 100.0,
            slippage_tolerance_bps: 10,
            fee_estimate_bps: 50,
            gas_cost_usd: 0.001,
            max_open_positions: 5,
            max_daily_loss: 100.0,
        };
        let publisher: Arc<dyn EventPublisher> = Arc::new(MockPublisher);
        let validator = TradeValidator::new(publisher, config);

        let opp = create_test_opportunity(100.0, 101.0);
        assert!(validator.check_risk_limits(&opp));
    }
}
