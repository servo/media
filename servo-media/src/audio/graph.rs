use audio::decoder::{AudioDecoder, AudioDecoderMsg};
use audio::graph_impl::{GraphImpl, InputPort, NodeId, OutputPort, PortId};
use audio::node::{AudioNodeMessage, AudioNodeType};
use audio::render_thread::AudioRenderThread;
use audio::render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_decoder::GStreamerAudioDecoder;

#[derive(Debug, PartialEq)]
pub enum ProcessingState {
    Suspended,
    Running,
    Closed,
}

pub struct AudioGraph {
    sender: Sender<AudioRenderThreadMsg>,
    state: ProcessingState,
    sample_rate: f32,
    dest_node: NodeId,
}

impl AudioGraph {
    pub fn new() -> Self {
        // XXX Get this from AudioContextOptions.
        let sample_rate = 44100.;

        let (sender, receiver) = mpsc::channel();
        let sender_ = sender.clone();
        let graph_impl = GraphImpl::new();
        let dest_node = graph_impl.dest_id();
        Builder::new()
            .name("AudioRenderThread".to_owned())
            .spawn(move || {
                AudioRenderThread::start(receiver, sender_, sample_rate, graph_impl)
                    .expect("Could not start AudioRenderThread");
            })
            .unwrap();
        Self {
            sender,
            state: ProcessingState::Suspended,
            sample_rate,
            dest_node,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn dest_node(&self) -> NodeId {
        self.dest_node
    }

    pub fn current_time(&self) -> f64 {
        let (sender, receiver) = mpsc::channel();
        let _ = self.sender
            .send(AudioRenderThreadMsg::GetCurrentTime(sender));
        receiver.recv().unwrap()
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> NodeId {
        let (tx, rx) = mpsc::channel();
        let _ = self.sender
            .send(AudioRenderThreadMsg::CreateNode(node_type, tx));
        rx.recv().unwrap()
    }

    /// Resume audio processing.
    pub fn resume(&mut self) {
        assert_eq!(self.state, ProcessingState::Suspended);
        self.state = ProcessingState::Running;
        let _ = self.sender.send(AudioRenderThreadMsg::Resume);
    }

    /// Suspend audio processing.
    pub fn suspend(&mut self) {
        self.state = ProcessingState::Suspended;
        let _ = self.sender.send(AudioRenderThreadMsg::Suspend);
    }

    /// Stop audio processing and close render thread.
    pub fn close(&mut self) {
        self.state = ProcessingState::Closed;
        let _ = self.sender.send(AudioRenderThreadMsg::Close);
    }

    pub fn message_node(&self, id: NodeId, msg: AudioNodeMessage) {
        let _ = self.sender.send(AudioRenderThreadMsg::MessageNode(id, msg));
    }

    pub fn connect_ports(&self, from: PortId<OutputPort>, to: PortId<InputPort>) {
        let _ = self.sender
            .send(AudioRenderThreadMsg::ConnectPorts(from, to));
    }

    /// Asynchronously decodes the audio file data contained in the given
    /// buffer.
    pub fn decode_audio_data(&self, data: Vec<u8>, sender: Sender<AudioDecoderMsg>) {
        Builder::new()
            .name("AudioDecoder".to_owned())
            .spawn(move || {
                #[cfg(feature = "gst")]
                let audio_decoder = GStreamerAudioDecoder::new();

                audio_decoder.decode(data, sender);
            })
            .unwrap();
    }
}

impl Drop for AudioGraph {
    fn drop(&mut self) {
        let _ = self.sender.send(AudioRenderThreadMsg::Close);
    }
}
