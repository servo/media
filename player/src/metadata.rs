use std::{string, time};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Metadata {
    pub duration: Option<time::Duration>,
    pub width: u32,
    pub height: u32,
    pub format: string::String,
    pub is_seekable: bool,
    // TODO: Might be nice to move width and height along with each video track.
    pub video_tracks: Vec<string::String>,
    pub audio_tracks: Vec<string::String>,
}
