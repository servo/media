use AudioStream;
use gst::prelude::*;
use super::gst_player;
use super::glib;

// XXX Define own error type.

pub struct GStreamerAudioStream {
    player: gst_player::Player,
}

impl GStreamerAudioStream {
    pub fn new() -> Result<Self, ()> {
        let player = gst_player::Player::new(None, None);
        player
            .set_property("uri", &glib::Value::from("webaudiosrc://foo"))
            .expect("Cant't set URI property");
        Ok(Self { player: player })
    }
}

impl AudioStream for GStreamerAudioStream {
    fn play(&self) {
        self.player.play();
    }

    fn stop(&self) {
        self.player.stop();
    }
}

impl Drop for GStreamerAudioStream {
    fn drop(&mut self) {
        self.stop();
    }
}
