use crate::domain::events::*;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    timestamp: i64,
    opportunities_detected: u64,
    trades_executed: u64,
    trades_rejected: u64,
    total_pnl_realized: f64,
    total_fees_paid: f64,
    net_profit: f64,
}

pub struct MetricsCollector {
    opportunities_detected: Arc<AtomicU64>,
    trades_executed: Arc<AtomicU64>,
    trades_rejected: Arc<AtomicU64>,
    total_pnl_realized: Arc<atomic_float::AtomicF64>,
    total_fees_paid: Arc<atomic_float::AtomicF64>,
    output_path: String,
}

impl MetricsCollector {
    pub fn new(output_path: String) -> Self {
        Self {
            opportunities_detected: Arc::new(AtomicU64::new(0)),
            trades_executed: Arc::new(AtomicU64::new(0)),
            trades_rejected: Arc::new(AtomicU64::new(0)),
            total_pnl_realized: Arc::new(atomic_float::AtomicF64::new(0.0)),
            total_fees_paid: Arc::new(atomic_float::AtomicF64::new(0.0)),
            output_path,
        }
    }

    pub async fn process_price_update(&self, _event: &PriceUpdate) {
    }

    pub async fn process_opportunity_detected(&self, _event: &OpportunityDetected) {
        let count = self.opportunities_detected.fetch_add(1, Ordering::Relaxed) + 1;
        info!("Opportunities detected: {}", count);
    }

    pub async fn process_trade_intent(&self, event: &TradeIntent) {
        info!(
            "Trade intent: {} expected profit ${:.2}",
            event.opportunity_id, event.expected_profit.net
        );
    }

    pub async fn process_execution_request(&self, event: &ExecutionRequest) {
        info!(
            "Execution request: {} {} -> {} amount={:.2}",
            event.trade_id, event.entry_dex, event.exit_dex, event.amount
        );
    }

    pub async fn process_trade_filled(&self, event: &TradeFilled) {
        let count = self.trades_executed.fetch_add(1, Ordering::Relaxed) + 1;
        self.total_pnl_realized
            .fetch_add(event.actual_profit.realized, Ordering::Relaxed);
        self.total_fees_paid
            .fetch_add(event.actual_profit.fees_paid, Ordering::Relaxed);

        info!(
            "Trade filled: {} realized=${:.2} fees=${:.2} total trades: {}",
            event.trade_id, event.actual_profit.realized, event.actual_profit.fees_paid, count
        );

        self.write_snapshot().await;
    }

    pub async fn process_trade_rejected(&self, event: &TradeRejected) {
        let count = self.trades_rejected.fetch_add(1, Ordering::Relaxed) + 1;
        info!(
            "Trade rejected: {} reason: {} total rejections: {}",
            event.trade_id, event.reason, count
        );
    }

    pub async fn process_position_update(&self, event: &PositionUpdate) {
        info!(
            "Position update: {} amount={:.2} unrealized_pnl=${:.2}",
            event.position.asset,
            event.position.amount,
            event.position.unrealized_pnl.net
        );
    }

    async fn write_snapshot(&self) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let snapshot = MetricsSnapshot {
            timestamp,
            opportunities_detected: self.opportunities_detected.load(Ordering::Relaxed),
            trades_executed: self.trades_executed.load(Ordering::Relaxed),
            trades_rejected: self.trades_rejected.load(Ordering::Relaxed),
            total_pnl_realized: self.total_pnl_realized.load(Ordering::Relaxed),
            total_fees_paid: self.total_fees_paid.load(Ordering::Relaxed),
            net_profit: self.total_pnl_realized.load(Ordering::Relaxed)
                - self.total_fees_paid.load(Ordering::Relaxed),
        };

        if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
            let _ = self.append_to_file(&json);
        }
    }

    fn append_to_file(&self, content: &str) -> std::io::Result<()> {
        if Path::new(&self.output_path).parent().is_some_and(|p| !p.as_os_str().is_empty()) {
            if let Some(parent) = Path::new(&self.output_path).parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        writeln!(file, "{}", content)?;
        writeln!(file)?;
        Ok(())
    }

    pub fn get_metrics(&self) -> MetricsSnapshot {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        MetricsSnapshot {
            timestamp,
            opportunities_detected: self.opportunities_detected.load(Ordering::Relaxed),
            trades_executed: self.trades_executed.load(Ordering::Relaxed),
            trades_rejected: self.trades_rejected.load(Ordering::Relaxed),
            total_pnl_realized: self.total_pnl_realized.load(Ordering::Relaxed),
            total_fees_paid: self.total_fees_paid.load(Ordering::Relaxed),
            net_profit: self.total_pnl_realized.load(Ordering::Relaxed)
                - self.total_fees_paid.load(Ordering::Relaxed),
        }
    }
}
