pub struct AudioDecoderCallbacks {
    pub eos: Option<Box<Fn() + Send + 'static>>,
    pub error: Option<Box<Fn() + Send + 'static>>,
    pub progress: Option<Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>>,
}

impl AudioDecoderCallbacks {
    pub fn new() -> AudioDecoderCallbacksBuilder {
        AudioDecoderCallbacksBuilder {
            eos: None,
            error: None,
            progress: None,
        }
    }

    pub fn eos(&self) {
        match self.eos {
            None => return,
            Some(ref callback) => callback(),
        };
    }

    pub fn error(&self) {
        match self.error {
            None => return,
            Some(ref callback) => callback(),
        };
    }

    pub fn progress(&self, buffer: Box<AsRef<[f32]>>) {
        match self.progress {
            None => return,
            Some(ref callback) => callback(buffer),
        };
    }
}

unsafe impl Send for AudioDecoderCallbacks {}
unsafe impl Sync for AudioDecoderCallbacks {}

pub struct AudioDecoderCallbacksBuilder {
    eos: Option<Box<Fn() + Send + 'static>>,
    error: Option<Box<Fn() + Send + 'static>>,
    progress: Option<Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>>,
}

impl AudioDecoderCallbacksBuilder {
    pub fn eos<F: Fn() + Send + 'static>(self, eos: F) -> Self {
        Self {
            eos: Some(Box::new(eos)),
            ..self
        }
    }

    pub fn error<F: Fn() + Send + 'static>(self, error: F) -> Self {
        Self {
            error: Some(Box::new(error)),
            ..self
        }
    }

    pub fn progress<F: Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>(self, progress: F) -> Self {
        Self {
            progress: Some(Box::new(progress)),
            ..self
        }
    }

    pub fn build(self) -> AudioDecoderCallbacks {
        AudioDecoderCallbacks {
            eos: self.eos,
            error: self.error,
            progress: self.progress,
        }
    }
}

pub enum AudioDecoderMsg {
    Eos,
    Error,
    // XXX Avoid copying :\
    Progress(Vec<f32>),
}

pub trait AudioDecoder {
    fn decode(&self, data: Vec<u8>, callbacks: AudioDecoderCallbacks);
}
