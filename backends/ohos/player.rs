use servo_media::MediaInstance;
use servo_media_player::Player;

pub struct OhosAVPlayer {}

impl OhosAVPlayer {
    pub fn new() -> OhosAVPlayer {
        OhosAVPlayer {}
    }
}

impl MediaInstance for OhosAVPlayer {
    fn get_id(&self) -> usize {
        todo!()
    }

    fn mute(&self, val: bool) -> Result<(), ()> {
        todo!()
    }

    fn suspend(&self) -> Result<(), ()> {
        todo!()
    }

    fn resume(&self) -> Result<(), ()> {
        todo!()
    }
}

impl Player for OhosAVPlayer {
    fn play(&self) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn pause(&self) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn stop(&self) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn seek(&self, time: f64) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn seekable(&self) -> Result<Vec<std::ops::Range<f64>>, servo_media_player::PlayerError> {
        todo!()
    }

    fn set_mute(&self, val: bool) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn set_volume(&self, value: f64) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn set_input_size(&self, size: u64) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn set_rate(&self, rate: f64) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn push_data(&self, data: Vec<u8>) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn end_of_stream(&self) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn buffered(&self) -> Result<Vec<std::ops::Range<f64>>, servo_media_player::PlayerError> {
        todo!()
    }

    fn set_stream(
        &self,
        stream: &servo_media_streams::MediaStreamId,
        only_stream: bool,
    ) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn render_use_gl(&self) -> bool {
        todo!()
    }

    fn set_audio_track(
        &self,
        stream_index: i32,
        enabled: bool,
    ) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }

    fn set_video_track(
        &self,
        stream_index: i32,
        enabled: bool,
    ) -> Result<(), servo_media_player::PlayerError> {
        todo!()
    }
}
