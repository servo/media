use super::MediaStream;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use uuid::Uuid;

type RegisteredMediaStream = Arc<Mutex<dyn MediaStream>>;

static MEDIA_STREAMS_REGISTRY: LazyLock<Mutex<HashMap<MediaStreamId, RegisteredMediaStream>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct MediaStreamId(Uuid);
impl MediaStreamId {
    pub fn new() -> MediaStreamId {
        Self { 0: Uuid::new_v4() }
    }

    pub fn id(self) -> Uuid {
        self.0
    }
}

pub fn register_stream(stream: Arc<Mutex<dyn MediaStream>>) -> MediaStreamId {
    let id = MediaStreamId::new();
    stream.lock().set_id(id.clone());
    MEDIA_STREAMS_REGISTRY.lock().insert(id.clone(), stream);
    id
}

pub fn unregister_stream(stream: &MediaStreamId) {
    MEDIA_STREAMS_REGISTRY.lock().remove(stream);
}

pub fn get_stream(stream: &MediaStreamId) -> Option<Arc<Mutex<dyn MediaStream>>> {
    MEDIA_STREAMS_REGISTRY.lock().get(stream).cloned()
}
