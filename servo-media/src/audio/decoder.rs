use sync::mpsc::Sender;

pub enum AudioDecoderMsg {
    Eos,
    Error,
    // XXX Avoid copying :\
    Progress(Vec<f32>),
}

pub trait AudioDecoder {
    fn decode(&self, data: Vec<u8>, sender: Sender<AudioDecoderMsg>);
}
