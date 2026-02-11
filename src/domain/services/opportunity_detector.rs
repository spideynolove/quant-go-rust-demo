use crate::domain::entities::{ArbitrageOpportunity, Dex, PnL, Price, Spread};
use crate::domain::events::{EventPublisher, OpportunityDetected, PriceUpdate};
use crate::infrastructure::{publish_event, OPPORTUNITIES};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const PRICE_STALENESS_MS: i64 = 5000;

pub struct OpportunityDetector {
    publisher: Arc<dyn EventPublisher>,
    min_spread_bps: u64,
    price_cache: HashMap<String, HashMap<Dex, CachedPrice>>,
}

struct CachedPrice {
    price: f64,
    timestamp: i64,
}

impl OpportunityDetector {
    pub fn new(publisher: Arc<dyn EventPublisher>, min_spread_bps: u64) -> Self {
        Self {
            publisher,
            min_spread_bps,
            price_cache: HashMap::new(),
        }
    }

    pub async fn process_price_update(&mut self, update: PriceUpdate) -> anyhow::Result<()> {
        let asset_pair = format!("{}:{}", update.base_asset, update.quote_asset);

        self.update_cache(&update, &asset_pair);
        self.cleanup_stale_prices();
        self.check_opportunities(&update, &asset_pair).await?;

        Ok(())
    }

    fn update_cache(&mut self, update: &PriceUpdate, asset_pair: &str) {
        let cached = CachedPrice {
            price: update.price,
            timestamp: update.timestamp,
        };
        self.price_cache
            .entry(asset_pair.to_string())
            .or_default()
            .insert(update.dex, cached);
    }

    fn cleanup_stale_prices(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for dex_prices in self.price_cache.values_mut() {
            dex_prices.retain(|_, cached| now - cached.timestamp < PRICE_STALENESS_MS);
        }
        self.price_cache.retain(|_, dex_prices| !dex_prices.is_empty());
    }

    async fn check_opportunities(
        &self,
        _update: &PriceUpdate,
        asset_pair: &str,
    ) -> anyhow::Result<()> {
        let dex_prices = match self.price_cache.get(asset_pair) {
            Some(prices) if prices.len() >= 2 => prices,
            _ => return Ok(()),
        };

        let dexes: Vec<(&Dex, &CachedPrice)> = dex_prices.iter().collect();
        for i in 0..dexes.len() {
            for j in (i + 1)..dexes.len() {
                let (dex_a, price_a) = dexes[i];
                let (dex_b, price_b) = dexes[j];

                let (buy_dex, sell_dex, buy_price, sell_price) = if price_a.price < price_b.price {
                    (*dex_a, *dex_b, price_a.price, price_b.price)
                } else {
                    (*dex_b, *dex_a, price_b.price, price_a.price)
                };

                let spread_bps = calculate_spread_bps(buy_price, sell_price);

                if spread_bps >= self.min_spread_bps {
                    let opportunity = self.create_opportunity(
                        buy_dex, sell_dex, buy_price, sell_price, spread_bps, asset_pair,
                    );

                    info!(
                        "Detected opportunity: {} {} -> {}, spread: {} bps",
                        asset_pair, buy_dex, sell_dex, spread_bps
                    );

                    let payload = publish_event(&OpportunityDetected { opportunity })?;
                    self.publisher.publish(OPPORTUNITIES, &payload).await?;
                }
            }
        }

        Ok(())
    }

    fn create_opportunity(
        &self,
        buy_dex: Dex,
        sell_dex: Dex,
        buy_price: f64,
        sell_price: f64,
        spread_bps: u64,
        asset_pair: &str,
    ) -> ArbitrageOpportunity {
        let parts: Vec<&str> = asset_pair.split(':').collect();
        let absolute = sell_price - buy_price;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        ArbitrageOpportunity {
            id: format!("{}_{}_{}_{}_{}", asset_pair, buy_dex, sell_dex, spread_bps, now),
            asset_pair: (parts[0].to_string(), parts[1].to_string()),
            buy_dex,
            sell_dex,
            buy_price: Price {
                value: buy_price,
                timestamp: now,
            },
            sell_price: Price {
                value: sell_price,
                timestamp: now,
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
            timestamp: now,
        }
    }
}

fn calculate_spread_bps(buy_price: f64, sell_price: f64) -> u64 {
    if buy_price > 0.0 {
        ((sell_price - buy_price) / buy_price * 10000.0) as u64
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_spread_bps() {
        assert_eq!(calculate_spread_bps(100.0, 100.5), 50);
        assert_eq!(calculate_spread_bps(100.0, 101.0), 100);
        assert_eq!(calculate_spread_bps(100.0, 102.0), 200);
        assert_eq!(calculate_spread_bps(0.0, 1.0), 0);
    }

    #[test]
    fn test_calculate_spread_bps_negative() {
        assert_eq!(calculate_spread_bps(100.0, 99.5), 0);
    }

    struct MockPublisher {
        published: std::sync::Mutex<Vec<(String, Vec<u8>)>>,
    }

    impl MockPublisher {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                published: std::sync::Mutex::new(Vec::new()),
            })
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

    #[tokio::test]
    async fn test_cross_dex_opportunity_detection() {
        let publisher = MockPublisher::new();
        let mut detector = OpportunityDetector::new(publisher.clone(), 50);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let raydium_update = PriceUpdate {
            pool_address: "ray_pool".to_string(),
            dex: Dex::Raydium,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 100.0,
            liquidity: crate::domain::entities::Liquidity {
                base_amount: 1000.0,
                quote_amount: 100000.0,
            },
            timestamp: now,
        };
        detector.process_price_update(raydium_update).await.unwrap();

        let orca_update = PriceUpdate {
            pool_address: "orca_pool".to_string(),
            dex: Dex::Orca,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 101.0,
            liquidity: crate::domain::entities::Liquidity {
                base_amount: 1000.0,
                quote_amount: 101000.0,
            },
            timestamp: now,
        };
        detector.process_price_update(orca_update).await.unwrap();

        let published = publisher.published.lock().unwrap();
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, OPPORTUNITIES);

        let event: OpportunityDetected = serde_json::from_slice(&published[0].1).unwrap();
        assert_eq!(event.opportunity.buy_dex, Dex::Raydium);
        assert_eq!(event.opportunity.sell_dex, Dex::Orca);
        assert_eq!(event.opportunity.spread.basis_points, 100);
    }

    #[tokio::test]
    async fn test_same_dex_no_opportunity() {
        let publisher = MockPublisher::new();
        let mut detector = OpportunityDetector::new(publisher.clone(), 50);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for price in [100.0, 101.0] {
            let update = PriceUpdate {
                pool_address: "pool".to_string(),
                dex: Dex::Raydium,
                base_asset: "SOL".to_string(),
                quote_asset: "USDC".to_string(),
                price,
                liquidity: crate::domain::entities::Liquidity {
                    base_amount: 1000.0,
                    quote_amount: 100000.0,
                },
                timestamp: now,
            };
            detector.process_price_update(update).await.unwrap();
        }

        let published = publisher.published.lock().unwrap();
        assert_eq!(published.len(), 0);
    }

    #[tokio::test]
    async fn test_spread_below_threshold_no_opportunity() {
        let publisher = MockPublisher::new();
        let mut detector = OpportunityDetector::new(publisher.clone(), 50);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let raydium = PriceUpdate {
            pool_address: "ray_pool".to_string(),
            dex: Dex::Raydium,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 100.0,
            liquidity: crate::domain::entities::Liquidity {
                base_amount: 1000.0,
                quote_amount: 100000.0,
            },
            timestamp: now,
        };
        detector.process_price_update(raydium).await.unwrap();

        let orca = PriceUpdate {
            pool_address: "orca_pool".to_string(),
            dex: Dex::Orca,
            base_asset: "SOL".to_string(),
            quote_asset: "USDC".to_string(),
            price: 100.2,
            liquidity: crate::domain::entities::Liquidity {
                base_amount: 1000.0,
                quote_amount: 100200.0,
            },
            timestamp: now,
        };
        detector.process_price_update(orca).await.unwrap();

        let published = publisher.published.lock().unwrap();
        assert_eq!(published.len(), 0);
    }
}
