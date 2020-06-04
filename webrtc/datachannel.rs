use crate::{WebRtcError, WebRtcResult};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

lazy_static! {
    static ref DATA_CHANNELS_REGISTRY: Mutex<HashMap<DataChannelId, Arc<Mutex<dyn DataChannelBackend>>>> =
        Mutex::new(HashMap::new());
}

pub trait DataChannelBackend: Send {
    fn send(&self, _: &str) -> WebRtcResult;
    fn close(&self) -> WebRtcResult;
}

pub enum DataChannelEvent {
    NewChannel,
    Open,
    Close,
    Error(WebRtcError),
    OnMessage(String),
}

// https://www.w3.org/TR/webrtc/#dom-rtcdatachannelinit
// plus `label`
pub struct DataChannelInit {
    pub label: String,
    pub ordered: bool,
    pub max_packet_life_time: Option<u16>,
    pub max_retransmits: Option<u16>,
    pub protocol: String,
    pub negotiated: bool,
    pub id: Option<u16>,
}

impl Default for DataChannelInit {
    fn default() -> DataChannelInit {
        DataChannelInit {
            label: Uuid::new_v4().to_string(),
            ordered: true,
            max_packet_life_time: None,
            max_retransmits: None,
            protocol: String::new(),
            negotiated: false,
            id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DataChannelId(Uuid);
impl DataChannelId {
    pub fn new() -> DataChannelId {
        Self { 0: Uuid::new_v4() }
    }

    pub fn id(&self) -> Uuid {
        self.0
    }
}

pub fn register_channel(
    id: &DataChannelId,
    channel: Arc<Mutex<dyn DataChannelBackend>>,
) -> Result<(), ()> {
    if DATA_CHANNELS_REGISTRY.lock().unwrap().contains_key(id) {
        return Err(());
    }
    DATA_CHANNELS_REGISTRY
        .lock()
        .unwrap()
        .insert(id.clone(), channel);
    Ok(())
}

pub fn unregister_channel(id: &DataChannelId) {
    DATA_CHANNELS_REGISTRY.lock().unwrap().remove(id);
}

pub fn get_channel(id: &DataChannelId) -> Option<Arc<Mutex<dyn DataChannelBackend>>> {
    DATA_CHANNELS_REGISTRY.lock().unwrap().get(id).cloned()
}
