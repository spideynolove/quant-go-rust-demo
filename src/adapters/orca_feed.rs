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
struct OrcaSubscribeRequest {
    pub action: String,
    pub channel: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OrcaPriceResponse {
    pub address: String,
    pub token_a: String,
    pub token_b: String,
    pub token_a_amount: f64,
    pub token_b_amount: f64,
    pub tick_current_index: i32,
    pub sqrt_price: u128,
}

pub struct OrcaFeed {
    publisher: Arc<NatsEventPublisher>,
    ws_url: String,
}

impl OrcaFeed {
    pub fn new(publisher: Arc<NatsEventPublisher>, ws_url: String) -> Self {
        Self { publisher, ws_url }
    }

    pub async fn run(&self) -> Result<()> {
        let mut reconnect_delay = Duration::from_secs(1);

        loop {
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("Orca feed connection closed gracefully");
                    break;
                }
                Err(e) => {
                    error!(
                        "Orca feed error: {}, reconnecting in {:?}",
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
        info!("Connecting to Orca WebSocket: {}", self.ws_url);
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let subscribe_msg = OrcaSubscribeRequest {
            action: "subscribe".to_string(),
            channel: "whirlpool".to_string(),
        };
        let payload = serde_json::to_string(&subscribe_msg)?;
        write.send(Message::Text(payload)).await?;

        info!("Subscribed to Orca whirlpool updates");

        let mut last_heartbeat = Instant::now();
        let heartbeat_interval = Duration::from_secs(30);

        while let Some(msg) = read.next().await {
            match msg? {
                Message::Text(text) => {
                    if let Err(e) = self.handle_message(&text).await {
                        warn!("Failed to handle Orca message: {}", e);
                    }
                }
                Message::Ping(data) => {
                    write.send(Message::Pong(data)).await?;
                }
                Message::Close(_) => {
                    info!("Orca WebSocket closed");
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
        if let Ok(response) = serde_json::from_str::<OrcaPriceResponse>(text) {
            let address = response.address.clone();
            let price = calculate_price(
                response.token_a_amount,
                response.token_b_amount,
                response.sqrt_price,
            );

            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            let price_update = PriceUpdate {
                pool_address: response.address,
                dex: Dex::Orca,
                base_asset: response.token_a,
                quote_asset: response.token_b,
                price,
                liquidity: Liquidity {
                    base_amount: response.token_a_amount,
                    quote_amount: response.token_b_amount,
                },
                timestamp,
            };

            let payload = publish_event(&price_update)?;
            self.publisher.publish(PRICE_UPDATES, &payload).await?;

            trace!("Published Orca price update: {}", address);
        }

        Ok(())
    }
}

fn calculate_price(token_a_amount: f64, token_b_amount: f64, sqrt_price: u128) -> f64 {
    if token_a_amount > 0.0 {
        token_b_amount / token_a_amount
    } else {
        let sqrt = sqrt_price as f64 / u64::MAX as f64;
        sqrt * sqrt
    }
}
