use crate::domain::events::{EventPublisher, ExecutionRequest, TradeIntent};
use crate::infrastructure::{publish_event, EXECUTION_REQUESTS};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub struct ExecutionCoordinator {
    publisher: Arc<dyn EventPublisher>,
}

impl ExecutionCoordinator {
    pub fn new(publisher: Arc<dyn EventPublisher>) -> Self {
        Self { publisher }
    }

    pub async fn coordinate_trade(&self, intent: TradeIntent) -> anyhow::Result<()> {
        let trade_id = format!(
            "trade_{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis()
        );

        info!(
            "Coordinating trade {} for opportunity {} ({} -> {}, amount={:.2})",
            trade_id, intent.opportunity_id, intent.buy_dex, intent.sell_dex, intent.amount
        );

        let parts: Vec<&str> = intent.opportunity_id.split('_').collect();
        let asset_pair = if let Some(pair) = parts.first() {
            let ap: Vec<&str> = pair.split(':').collect();
            if ap.len() == 2 {
                (ap[0].to_string(), ap[1].to_string())
            } else {
                ("SOL".to_string(), "USDC".to_string())
            }
        } else {
            ("SOL".to_string(), "USDC".to_string())
        };

        let execution_request = ExecutionRequest {
            trade_id: trade_id.clone(),
            entry_dex: intent.buy_dex,
            exit_dex: intent.sell_dex,
            amount: intent.amount,
            asset_pair,
        };

        let payload = publish_event(&execution_request)?;
        self.publisher
            .publish(EXECUTION_REQUESTS, &payload)
            .await?;

        info!("Trade {} execution request sent", trade_id);

        Ok(())
    }
}
