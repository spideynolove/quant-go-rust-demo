use crate::domain::entities::{PnL, Position};
use crate::domain::events::{EventPublisher, PositionUpdate, TradeFilled};
use crate::infrastructure::{publish_event, POSITION_UPDATES};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

pub struct PositionTracker {
    publisher: Arc<dyn EventPublisher>,
    positions: HashMap<String, TrackedPosition>,
}

struct TrackedPosition {
    asset: String,
    amount: f64,
    entry_price: f64,
    current_price: f64,
    realized_pnl: f64,
    fees_paid: f64,
}

impl PositionTracker {
    pub fn new(publisher: Arc<dyn EventPublisher>) -> Self {
        Self {
            publisher,
            positions: HashMap::new(),
        }
    }

    pub async fn process_trade_filled(&mut self, event: TradeFilled) -> anyhow::Result<()> {
        info!("Processing trade filled: {}", event.trade_id);

        let position_update = self.update_position(&event);

        let payload = publish_event(&position_update)?;
        self.publisher
            .publish(POSITION_UPDATES, &payload)
            .await?;

        Ok(())
    }

    fn update_position(&mut self, event: &TradeFilled) -> PositionUpdate {
        let asset = event.asset_pair.0.clone();

        let tracked = self.positions.entry(asset.clone()).or_insert_with(|| {
            info!("Creating new position for {}", asset);
            TrackedPosition {
                asset: asset.clone(),
                amount: 0.0,
                entry_price: event.entry_price,
                current_price: event.exit_price,
                realized_pnl: 0.0,
                fees_paid: 0.0,
            }
        });

        tracked.amount += event.amount;
        tracked.current_price = event.exit_price;
        tracked.realized_pnl += event.actual_profit.realized;
        tracked.fees_paid += event.actual_profit.fees_paid;

        let position = Position {
            asset: tracked.asset.clone(),
            amount: tracked.amount,
            entry_price: tracked.entry_price,
            current_price: tracked.current_price,
            unrealized_pnl: PnL {
                realized: tracked.realized_pnl,
                fees_paid: tracked.fees_paid,
                net: tracked.realized_pnl - tracked.fees_paid,
            },
        };

        info!(
            "Position updated: {} amount={:.2} realized_pnl=${:.2}",
            tracked.asset, tracked.amount, tracked.realized_pnl
        );

        PositionUpdate { position }
    }

    pub fn get_position(&self, asset: &str) -> Option<Position> {
        self.positions.get(asset).map(|tracked| Position {
            asset: tracked.asset.clone(),
            amount: tracked.amount,
            entry_price: tracked.entry_price,
            current_price: tracked.current_price,
            unrealized_pnl: PnL {
                realized: tracked.realized_pnl,
                fees_paid: tracked.fees_paid,
                net: tracked.realized_pnl - tracked.fees_paid,
            },
        })
    }
}
