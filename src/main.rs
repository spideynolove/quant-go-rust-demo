use solana_arb::adapters::{MetricsCollector, OrcaFeed, RaydiumFeed, SolanaExecutor};
use solana_arb::domain::events::*;
use solana_arb::domain::services::{
    ExecutionCoordinator, OpportunityDetector, PositionTracker, TradeValidator, ValidatorConfig,
};
use solana_arb::infrastructure::{
    connect, load_config_with_env, subscribe, NatsEventPublisher,
    EXECUTION_REQUESTS, OPPORTUNITIES, PRICE_UPDATES, TRADE_FILLED, TRADE_INTENTS,
    TRADE_REJECTED, POSITION_UPDATES,
};
use anyhow::Result;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config_with_env("config.toml")?;
    init_logging(&config.logging);

    info!("Solana DEX Arbitrage Bot starting...");

    let nats_client = connect(&config.nats.url).await?;
    info!("Connected to NATS at {}", config.nats.url);

    let publisher = NatsEventPublisher::new(nats_client.clone());

    let mut tasks = JoinSet::new();

    let metrics = Arc::new(MetricsCollector::new("metrics.jsonl".to_string()));

    let opportunity_detector = Arc::new(RwLock::new(OpportunityDetector::new(
        publisher.clone(),
        config.trading.min_spread_bps,
    )));

    let trade_validator = Arc::new(TradeValidator::new(
        publisher.clone(),
        ValidatorConfig {
            min_profit_usd: config.trading.min_profit_usd,
            max_position_size: config.trading.max_position_size,
            max_trade_size: config.trading.max_trade_size,
            slippage_tolerance_bps: config.trading.slippage_tolerance_bps,
            fee_estimate_bps: 50,
            gas_cost_usd: 0.001,
            max_open_positions: config.risk.max_open_positions,
            max_daily_loss: config.risk.max_daily_loss,
        },
    ));

    let execution_coordinator = Arc::new(ExecutionCoordinator::new(publisher.clone()));

    let position_tracker = Arc::new(RwLock::new(PositionTracker::new(publisher.clone())));

    let solana_executor = match SolanaExecutor::new(
        publisher.clone(),
        config.solana.rpc_url.clone(),
        config.solana.keypair_path.clone(),
    ) {
        Ok(executor) => {
            info!("SolanaExecutor initialized");
            Some(Arc::new(executor))
        }
        Err(e) => {
            warn!("SolanaExecutor not available (keypair not configured): {}", e);
            None
        }
    };

    {
        let raydium_publisher = publisher.clone();
        let ws_url = config.dexs.raydium.websocket_url.clone();
        tasks.spawn(async move {
            let feed = RaydiumFeed::new(raydium_publisher, ws_url);
            if let Err(e) = feed.run().await {
                error!("Raydium feed error: {}", e);
            }
        });
    }

    {
        let orca_publisher = publisher.clone();
        let ws_url = config.dexs.orca.websocket_url.clone();
        tasks.spawn(async move {
            let feed = OrcaFeed::new(orca_publisher, ws_url);
            if let Err(e) = feed.run().await {
                error!("Orca feed error: {}", e);
            }
        });
    }

    {
        let price_sub = subscribe(&nats_client, PRICE_UPDATES).await?;
        let detector = opportunity_detector.clone();
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(price_sub, move |payload| {
                let detector = detector.clone();
                let m = m.clone();
                async move {
                    if let Ok(update) = serde_json::from_slice::<PriceUpdate>(&payload) {
                        m.process_price_update(&update).await;
                        let mut det = detector.write().await;
                        if let Err(e) = det.process_price_update(update).await {
                            warn!("Failed to process price update: {}", e);
                        }
                    }
                }
            })
            .await;
        });
    }

    {
        let opp_sub = subscribe(&nats_client, OPPORTUNITIES).await?;
        let validator = trade_validator.clone();
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(opp_sub, move |payload| {
                let validator = validator.clone();
                let m = m.clone();
                async move {
                    if let Ok(event) = serde_json::from_slice::<OpportunityDetected>(&payload) {
                        m.process_opportunity_detected(&event).await;
                        if let Err(e) = validator.validate_opportunity(event).await {
                            warn!("Failed to validate opportunity: {}", e);
                        }
                    }
                }
            })
            .await;
        });
    }

    {
        let intent_sub = subscribe(&nats_client, TRADE_INTENTS).await?;
        let coordinator = execution_coordinator.clone();
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(intent_sub, move |payload| {
                let coordinator = coordinator.clone();
                let m = m.clone();
                async move {
                    if let Ok(intent) = serde_json::from_slice::<TradeIntent>(&payload) {
                        m.process_trade_intent(&intent).await;
                        if let Err(e) = coordinator.coordinate_trade(intent).await {
                            warn!("Failed to coordinate trade: {}", e);
                        }
                    }
                }
            })
            .await;
        });
    }

    if let Some(executor) = solana_executor {
        let exec_sub = subscribe(&nats_client, EXECUTION_REQUESTS).await?;
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(exec_sub, move |payload| {
                let executor = executor.clone();
                let m = m.clone();
                async move {
                    if let Ok(request) = serde_json::from_slice::<ExecutionRequest>(&payload) {
                        m.process_execution_request(&request).await;
                        if let Err(e) = executor.execute_trade(request).await {
                            warn!("Failed to execute trade: {}", e);
                        }
                    }
                }
            })
            .await;
        });
    }

    {
        let filled_sub = subscribe(&nats_client, TRADE_FILLED).await?;
        let tracker = position_tracker.clone();
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(filled_sub, move |payload| {
                let tracker = tracker.clone();
                let m = m.clone();
                async move {
                    if let Ok(event) = serde_json::from_slice::<TradeFilled>(&payload) {
                        m.process_trade_filled(&event).await;
                        let mut t = tracker.write().await;
                        if let Err(e) = t.process_trade_filled(event).await {
                            warn!("Failed to process trade filled: {}", e);
                        }
                    }
                }
            })
            .await;
        });
    }

    {
        let rejected_sub = subscribe(&nats_client, TRADE_REJECTED).await?;
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(rejected_sub, move |payload| {
                let m = m.clone();
                async move {
                    if let Ok(event) = serde_json::from_slice::<TradeRejected>(&payload) {
                        m.process_trade_rejected(&event).await;
                    }
                }
            })
            .await;
        });
    }

    {
        let pos_sub = subscribe(&nats_client, POSITION_UPDATES).await?;
        let m = metrics.clone();
        tasks.spawn(async move {
            run_subscriber(pos_sub, move |payload| {
                let m = m.clone();
                async move {
                    if let Ok(event) = serde_json::from_slice::<PositionUpdate>(&payload) {
                        m.process_position_update(&event).await;
                    }
                }
            })
            .await;
        });
    }

    info!("All services started. Press Ctrl+C to shutdown gracefully.");

    signal::ctrl_c().await?;
    info!("Shutdown signal received");

    info!("Shutting down tasks...");
    tasks.shutdown().await;

    info!("Solana DEX Arbitrage Bot stopped");

    Ok(())
}

async fn run_subscriber<F, Fut>(mut subscriber: async_nats::Subscriber, handler: F)
where
    F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    while let Some(msg) = subscriber.next().await {
        handler(msg.payload.to_vec()).await;
    }
}

fn init_logging(config: &solana_arb::infrastructure::LoggingConfig) {
    let level = match config.level.to_lowercase().as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "warn" | "warning" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };

    match config.format.to_lowercase().as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .json()
                .with_max_level(level)
                .with_target(false)
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_max_level(level)
                .with_target(false)
                .init();
        }
    }
}
