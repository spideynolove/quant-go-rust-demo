use crate::domain::entities::{Dex, Liquidity};
use crate::domain::events::{EventPublisher, PriceUpdate};
use crate::infrastructure::{publish_event, NatsEventPublisher, PRICE_UPDATES};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, trace, warn};

const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize)]
struct RaydiumSubscribeRequest {
    pub subscribe: String,
}

#[derive(Debug, Deserialize)]
struct RaydiumPriceResponse {
    pub pool: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_amount: f64,
    pub quote_amount: f64,
    pub price: f64,
}

pub struct RaydiumFeed {
    publisher: Arc<NatsEventPublisher>,
    ws_url: String,
}

impl RaydiumFeed {
    pub fn new(publisher: Arc<NatsEventPublisher>, ws_url: String) -> Self {
        Self { publisher, ws_url }
    }

    pub async fn run(&self) -> Result<()> {
        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("Raydium feed connection closed gracefully");
                    break;
                }
                Err(e) => {
                    error!(
                        "Raydium feed error: {}, reconnecting in {:?}",
                        e, reconnect_delay
                    );
                    tokio::time::sleep(reconnect_delay).await;
                    reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY);
                }
            }
        }

        Ok(())
    }

    async fn connect_and_run(&self) -> Result<()> {
        info!("Connecting to Raydium WebSocket: {}", self.ws_url);
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let subscribe_msg = RaydiumSubscribeRequest {
            subscribe: "liquidity".to_string(),
        };
        let payload = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(payload)).await?;

        info!("Subscribed to Raydium liquidity updates");

        let mut last_heartbeat = Instant::now();
        let heartbeat_interval = Duration::from_secs(30);

        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    if let Err(e) = self.handle_message(&text).await {
                        warn!("Failed to handle Raydium message: {}", e);
                    }
                }
                Message::Ping(data) => {
                    write.send(Message::Pong(data)).await?;
                }
                Message::Close(_) => {
                    info!("Raydium WebSocket closed");
                    break;
                }
                _ => {}
            }

            if last_heartbeat.elapsed() >= heartbeat_interval {
                write.send(Message::Ping(vec![])).await?;
                last_heartbeat = Instant::now();
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        if let Ok(response) = serde_json::from_str::<RaydiumPriceResponse>(text) {
            let pool = response.pool.clone();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            let price_update = PriceUpdate {
                pool_address: response.pool,
                dex: Dex::Raydium,
                base_asset: response.base_mint,
                quote_asset: response.quote_mint,
                price: response.price,
                liquidity: Liquidity {
                    base_amount: response.base_amount,
                    quote_amount: response.quote_amount,
                },
                timestamp,
            };

            let payload = publish_event(&price_update)?;
            self.publisher.publish(PRICE_UPDATES, &payload).await?;

            trace!("Published Raydium price update: {}", pool);
        }

        Ok(())
    }
}
