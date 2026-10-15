//! Train Position Adapter
//!
//! Transform a train update XML message into vehicle location events.

use anyhow::Context as _;
use chrono::Utc;
use common::{block_mgt, config, routes};
use omnia_sdk::api::Context;
use omnia_sdk::{Config, HttpRequest, Identity, Message, Publish, Result};
use serde::Deserialize;

use crate::stops;
use crate::train_update::TrainUpdate;
use crate::vehicle_event::{EventType, MessageData, RemoteData, VehicleEvent};

/// Each vehicle event is published this many times: the downstream schedule
/// adherence process treats the repeated location event as confirmation that
/// the train has departed the station, so a single publish would leave
/// departures unsignalled.
///
/// The legacy system spaced the repeats five seconds apart with a blocking
/// `thread::sleep`. Do not copy that pattern — blocking stalls the entire
/// WASM guest. If spacing between publishes ever matters, use a scheduled or
/// delayed message, or debounce in the consumer, instead of sleeping in the
/// handler.
const PUBLISH_REPEATS: usize = 2;

/// Train update message as deserialized from the XML received from the
/// upstream rail feed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TrainMessage {
    /// The train update.
    #[serde(rename(deserialize = "ActualizarDatosTren"))]
    pub train_update: TrainUpdate,
}

impl TrainMessage {
    /// Deserialize a train update message from raw XML.
    ///
    /// # Errors
    ///
    /// Returns an error if the XML cannot be deserialized.
    pub fn from_xml(input: &[u8]) -> Result<Self> {
        quick_xml::de::from_reader(input).context("deserializing TrainMessage").map_err(Into::into)
    }
}

/// Validates a train update and publishes the resulting vehicle location
/// events to the train-positions topic.
///
/// # Errors
///
/// Returns an error when the update fails validation, a provider request
/// fails, or an event cannot be serialized or published.
#[tracing::instrument(skip_all)]
pub async fn train_update<P>(input: TrainMessage, context: Context<P>) -> Result<()>
where
    P: Config + HttpRequest + Identity + Publish,
{
    let provider = context.provider();

    // validate message
    let update = input.train_update;
    update.validate()?;

    // convert to vehicle events
    let events = update.into_events(context.owner(), provider).await?;

    // publish events to train-positions topic (repeated — see `PUBLISH_REPEATS`)
    let topic = config::topic(provider, routes::topic::TRAIN_POSITIONS).await;

    for _ in 0..PUBLISH_REPEATS {
        for event in &events {
            tracing::info!(monotonic_counter.vehicle_events_published = 1);

            let payload = serde_json::to_vec(&event).context("serializing event")?;
            let external_id = &event.remote_data.external_id;

            let mut message = Message::new(&payload);
            message.headers.insert("key".to_string(), external_id.clone());

            Publish::send(provider, &topic, &message).await?;
        }
    }

    Ok(())
}

impl TrainUpdate {
    /// Transform the train update message to vehicle location events.
    async fn into_events<P>(self, owner: &str, provider: &P) -> Result<Vec<VehicleEvent>>
    where
        P: Config + HttpRequest + Identity + Publish,
    {
        let changes = &self.changes;
        let change_type = changes[0].r#type;

        // filter out irrelevant updates (not related to trip progress)
        if !change_type.is_relevant() {
            tracing::info!(monotonic_counter.irrelevant_change_type = 1, type = %change_type);
            return Ok(vec![]);
        }

        // is station is relevant?
        let station = changes[0].station;
        let Some(stop_info) =
            stops::stop_info(owner, provider, station, change_type.is_arrival()).await?
        else {
            tracing::info!(monotonic_counter.irrelevant_station = 1, station = %station);
            return Ok(vec![]);
        };

        // get train allocations for this trip
        let allocated = block_mgt::trip_allocations(&self.train_id(), provider)
            .await
            .context("fetching train allocations")?;

        // publish vehicle events
        let mut events = Vec::new();
        for train in allocated {
            events.push(VehicleEvent {
                received_at: Utc::now(),
                event_type: EventType::Location,
                message_data: MessageData::default(),
                remote_data: RemoteData {
                    external_id: train.replace(' ', ""),
                    ..RemoteData::default()
                },
                location_data: stop_info.clone().into(),
                ..VehicleEvent::default()
            });
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::TrainMessage;

    #[test]
    fn deserialization() {
        let xml = include_str!("../data/sample.xml");
        let message: TrainMessage = quick_xml::de::from_str(xml).expect("should deserialize");

        let update = message.train_update;
        assert_eq!(update.even_train_id, Some("1234".to_string()));
        assert!(!update.changes.is_empty(), "should have changes");
    }
}
