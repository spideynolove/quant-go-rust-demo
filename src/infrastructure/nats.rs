use crate::domain::events::EventPublisher;
use anyhow::Result;
use async_nats::Client;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;

pub struct NatsEventPublisher {
    client: Client,
}

impl NatsEventPublisher {
    pub fn new(client: Client) -> Arc<Self> {
        Arc::new(Self { client })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[async_trait]
impl EventPublisher for NatsEventPublisher {
    async fn publish(&self, subject: &str, payload: &[u8]) -> Result<()> {
        self.client
            .publish(subject.to_string(), payload.to_vec().into())
            .await?;
        Ok(())
    }
}

pub async fn connect(url: &str) -> Result<Client> {
    let client = async_nats::connect(url).await?;
    Ok(client)
}

pub fn publish_event<T: Serialize>(event: &T) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(event)?)
}

pub async fn subscribe(client: &Client, subject: &str) -> Result<async_nats::Subscriber> {
    let subscriber = client.subscribe(subject.to_string()).await?;
    Ok(subscriber)
}

pub const PRICE_UPDATES: &str = "prices.updates";
pub const OPPORTUNITIES: &str = "opportunities.detected";
pub const TRADE_INTENTS: &str = "trades.intents";
pub const EXECUTION_REQUESTS: &str = "execution.requests";
pub const TRADE_FILLED: &str = "trades.filled";
pub const TRADE_REJECTED: &str = "trades.rejected";
pub const POSITION_UPDATES: &str = "positions.updates";
