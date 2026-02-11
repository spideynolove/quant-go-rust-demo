use solana_arb::domain::entities::*;
use solana_arb::domain::events::*;
use solana_arb::domain::services::*;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct MockPublisher {
    published: std::sync::Mutex<Vec<(String, Vec<u8>)>>,
}

impl MockPublisher {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            published: std::sync::Mutex::new(Vec::new()),
        })
    }

    fn get_published(&self) -> Vec<(String, Vec<u8>)> {
        self.published.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl EventPublisher for MockPublisher {
    async fn publish(&self, subject: &str, payload: &[u8]) -> anyhow::Result<()> {
        self.published
            .lock()
            .unwrap()
            .push((subject.to_string(), payload.to_vec()));
        Ok(())
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

#[tokio::test]
async fn test_end_to_end_opportunity_detection_and_validation() {
    let detector_pub = MockPublisher::new();
    let validator_pub = MockPublisher::new();

    let mut detector = OpportunityDetector::new(detector_pub.clone(), 50);
    let validator = TradeValidator::new(
        validator_pub.clone(),
        ValidatorConfig {
            min_profit_usd: 0.01,
            max_position_size: 1000.0,
            max_trade_size: 100.0,
            slippage_tolerance_bps: 10,
            fee_estimate_bps: 30,
            gas_cost_usd: 0.001,
            max_open_positions: 5,
            max_daily_loss: 100.0,
        },
    );

    let ts = now_ms();

    detector
        .process_price_update(PriceUpdate {
            pool_address: "ray_pool".to_string(),
            dex: Dex::Raydium,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 100.0,
            liquidity: Liquidity {
                base_amount: 10000.0,
                quote_amount: 1000000.0,
            },
            timestamp: ts,
        })
        .await
        .unwrap();

    detector
        .process_price_update(PriceUpdate {
            pool_address: "orca_pool".to_string(),
            dex: Dex::Orca,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 102.0,
            liquidity: Liquidity {
                base_amount: 10000.0,
                quote_amount: 1020000.0,
            },
            timestamp: ts,
        })
        .await
        .unwrap();

    let detected = detector_pub.get_published();
    assert_eq!(detected.len(), 1);

    let opp_event: OpportunityDetected = serde_json::from_slice(&detected[0].1).unwrap();
    assert_eq!(opp_event.opportunity.buy_dex, Dex::Raydium);
    assert_eq!(opp_event.opportunity.sell_dex, Dex::Orca);
    assert!(opp_event.opportunity.spread.basis_points >= 200);

    validator
        .validate_opportunity(opp_event)
        .await
        .unwrap();

    let intents = validator_pub.get_published();
    assert_eq!(intents.len(), 1);

    let intent: TradeIntent = serde_json::from_slice(&intents[0].1).unwrap();
    assert_eq!(intent.buy_dex, Dex::Raydium);
    assert_eq!(intent.sell_dex, Dex::Orca);
    assert!(intent.amount > 0.0);
    assert!(intent.expected_profit.net > 0.0);
}

#[tokio::test]
async fn test_event_serialization_roundtrip() {
    let opportunity = ArbitrageOpportunity {
        id: "test_opp_roundtrip".to_string(),
        asset_pair: ("SOL".to_string(), "USDC".to_string()),
        buy_dex: Dex::Raydium,
        sell_dex: Dex::Orca,
        buy_price: Price {
            value: 100.0,
            timestamp: 12345,
        },
        sell_price: Price {
            value: 101.5,
            timestamp: 12345,
        },
        spread: Spread {
            basis_points: 150,
            absolute: 1.5,
        },
        estimated_profit: PnL {
            realized: 0.0,
            fees_paid: 0.0,
            net: 0.0,
        },
        timestamp: 12345,
    };

    let event = OpportunityDetected {
        opportunity: opportunity.clone(),
    };
    let json = serde_json::to_vec(&event).unwrap();
    let deserialized: OpportunityDetected = serde_json::from_slice(&json).unwrap();

    assert_eq!(deserialized.opportunity.id, opportunity.id);
    assert_eq!(deserialized.opportunity.buy_dex, Dex::Raydium);
    assert_eq!(deserialized.opportunity.sell_dex, Dex::Orca);
    assert_eq!(deserialized.opportunity.spread.basis_points, 150);

    let intent = TradeIntent {
        opportunity_id: "opp_123".to_string(),
        buy_dex: Dex::Orca,
        sell_dex: Dex::Raydium,
        amount: 50.0,
        expected_profit: PnL {
            realized: 1.5,
            fees_paid: 0.3,
            net: 1.2,
        },
    };
    let json = serde_json::to_vec(&intent).unwrap();
    let deserialized: TradeIntent = serde_json::from_slice(&json).unwrap();
    assert_eq!(deserialized.buy_dex, Dex::Orca);
    assert_eq!(deserialized.amount, 50.0);

    let filled = TradeFilled {
        trade_id: "trade_1".to_string(),
        entry_dex: Dex::Raydium,
        exit_dex: Dex::Orca,
        amount: 100.0,
        asset_pair: ("SOL".to_string(), "USDC".to_string()),
        entry_price: 100.0,
        exit_price: 101.0,
        actual_profit: PnL {
            realized: 1.0,
            fees_paid: 0.1,
            net: 0.9,
        },
    };
    let json = serde_json::to_vec(&filled).unwrap();
    let deserialized: TradeFilled = serde_json::from_slice(&json).unwrap();
    assert_eq!(deserialized.entry_dex, Dex::Raydium);
    assert_eq!(deserialized.amount, 100.0);
}

#[tokio::test]
async fn test_position_tracker_accumulation() {
    let publisher = MockPublisher::new();
    let mut tracker = PositionTracker::new(publisher.clone());

    let fill1 = TradeFilled {
        trade_id: "trade_1".to_string(),
        entry_dex: Dex::Raydium,
        exit_dex: Dex::Orca,
        amount: 1.0,
        asset_pair: ("SOL".to_string(), "USDC".to_string()),
        entry_price: 100.0,
        exit_price: 101.0,
        actual_profit: PnL {
            realized: 1.0,
            fees_paid: 0.1,
            net: 0.9,
        },
    };
    tracker.process_trade_filled(fill1).await.unwrap();

    let fill2 = TradeFilled {
        trade_id: "trade_2".to_string(),
        entry_dex: Dex::Raydium,
        exit_dex: Dex::Orca,
        amount: 2.0,
        asset_pair: ("SOL".to_string(), "USDC".to_string()),
        entry_price: 101.0,
        exit_price: 102.0,
        actual_profit: PnL {
            realized: 2.0,
            fees_paid: 0.2,
            net: 1.8,
        },
    };
    tracker.process_trade_filled(fill2).await.unwrap();

    let position = tracker.get_position("SOL").unwrap();
    assert!((position.amount - 3.0).abs() < f64::EPSILON);
    assert!((position.unrealized_pnl.realized - 3.0).abs() < f64::EPSILON);
    assert!((position.unrealized_pnl.fees_paid - 0.3).abs() < f64::EPSILON);

    let published = publisher.get_published();
    assert_eq!(published.len(), 2);
}

#[tokio::test]
async fn test_execution_coordinator_passes_intent_data() {
    let publisher = MockPublisher::new();
    let coordinator = ExecutionCoordinator::new(publisher.clone());

    let intent = TradeIntent {
        opportunity_id: "SOL:USDC_Raydium_Orca_100_12345".to_string(),
        buy_dex: Dex::Orca,
        sell_dex: Dex::Raydium,
        amount: 75.0,
        expected_profit: PnL {
            realized: 1.5,
            fees_paid: 0.3,
            net: 1.2,
        },
    };

    coordinator.coordinate_trade(intent).await.unwrap();

    let published = publisher.get_published();
    assert_eq!(published.len(), 1);

    let request: ExecutionRequest = serde_json::from_slice(&published[0].1).unwrap();
    assert_eq!(request.entry_dex, Dex::Orca);
    assert_eq!(request.exit_dex, Dex::Raydium);
    assert!((request.amount - 75.0).abs() < f64::EPSILON);
}
