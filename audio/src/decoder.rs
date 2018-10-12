use boxfnonce::SendBoxFnOnce;
use std::fmt::Debug;
use std::sync::Mutex;

pub struct AudioDecoderCallbacks<E> {
    pub eos: Mutex<Option<SendBoxFnOnce<'static, ()>>>,
    pub error: Mutex<Option<SendBoxFnOnce<'static, (E,)>>>,
    pub progress: Option<Box<Fn(Box<AsRef<[f32]>>, u32) + Send + Sync + 'static>>,
    pub ready: Mutex<Option<SendBoxFnOnce<'static, (u32,)>>>,
}

impl<E> AudioDecoderCallbacks<E> {
    pub fn new() -> AudioDecoderCallbacksBuilder<E> {
        AudioDecoderCallbacksBuilder {
            eos: None,
            error: None,
            progress: None,
            ready: None,
        }
    }

    pub fn eos(&self) {
        let eos = self.eos.lock().unwrap().take();
        match eos {
            None => return,
            Some(callback) => callback.call(),
        };
    }

    pub fn error(&self, error: E) {
        let callback = self.error.lock().unwrap().take();
        match callback {
            None => return,
            Some(callback) => callback.call(error),
        };
    }

    pub fn progress(&self, buffer: Box<AsRef<[f32]>>, channel: u32) {
        match self.progress {
            None => return,
            Some(ref callback) => callback(buffer, channel),
        };
    }

    pub fn ready(&self, channels: u32) {
        let ready = self.ready.lock().unwrap().take();
        match ready {
            None => return,
            Some(callback) => callback.call(channels),
        };
    }
}

pub struct AudioDecoderCallbacksBuilder<E> {
    eos: Option<SendBoxFnOnce<'static, ()>>,
    error: Option<SendBoxFnOnce<'static, (E,)>>,
    progress: Option<Box<Fn(Box<AsRef<[f32]>>, u32) + Send + Sync + 'static>>,
    ready: Option<SendBoxFnOnce<'static, (u32,)>>,
}

impl<E> AudioDecoderCallbacksBuilder<E> {
    pub fn eos<F: FnOnce() + Send + 'static>(self, eos: F) -> Self {
        Self {
            eos: Some(SendBoxFnOnce::new(eos)),
            ..self
        }
    }

    pub fn error<F: FnOnce(E) + Send + 'static>(self, error: F) -> Self {
        Self {
            error: Some(SendBoxFnOnce::new(error)),
            ..self
        }
    }

    pub fn progress<F: Fn(Box<AsRef<[f32]>>, u32) + Send + Sync + 'static>(
        self,
        progress: F,
    ) -> Self {
        Self {
            progress: Some(Box::new(progress)),
            ..self
        }
    }

    pub fn ready<F: FnOnce(u32) + Send + 'static>(self, ready: F) -> Self {
        Self {
            ready: Some(SendBoxFnOnce::new(ready)),
            ..self
        }
    }

    pub fn build(self) -> AudioDecoderCallbacks<E> {
        AudioDecoderCallbacks {
            eos: Mutex::new(self.eos),
            error: Mutex::new(self.error),
            progress: self.progress,
            ready: Mutex::new(self.ready),
        }
    }
}

pub struct AudioDecoderOptions {
    pub sample_rate: f32,
}

impl Default for AudioDecoderOptions {
    fn default() -> Self {
        AudioDecoderOptions {
            sample_rate: 44100.,
        }
    }
}

pub trait AudioDecoder {
    type Error: Debug;
    fn decode(
        &self,
        data: Vec<u8>,
        callbacks: AudioDecoderCallbacks<Self::Error>,
        options: Option<AudioDecoderOptions>,
    );
}

pub struct DummyAudioDecoder;

impl AudioDecoder for DummyAudioDecoder {
    type Error = ();
    fn decode(&self, _: Vec<u8>, _: AudioDecoderCallbacks<()>, _: Option<AudioDecoderOptions>) {}
}
