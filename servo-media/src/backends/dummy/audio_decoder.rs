use audio::decoder::{AudioDecoder, AudioDecoderCallbacks, AudioDecoderOptions};

pub struct DummyAudioDecoder {}

impl AudioDecoder for DummyAudioDecoder {
    fn decode(&self, _: Vec<u8>, _: AudioDecoderCallbacks, _: Option<AudioDecoderOptions>) {}
}
