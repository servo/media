use audio::decoder::{AudioDecoder, AudioDecoderCallbacks, AudioDecoderOptions};
use audio::graph_impl::{GraphImpl, InputPort, NodeId, OutputPort, PortId};
use audio::node::{AudioNodeMessage, AudioNodeType};
use audio::render_thread::AudioRenderThread;
use audio::render_thread::AudioRenderThreadMsg;
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

#[cfg(feature = "gst")]
use backends::gstreamer::audio_decoder::GStreamerAudioDecoder;

/// Describes the state of the audio context on the control thread.
#[derive(Debug, PartialEq)]
pub enum ProcessingState {
    Suspended,
    Running,
    Closed,
}

/// Identify the type of playback, which affects tradeoffs between audio output
/// and power consumption.
pub enum LatencyCategory {
    /// Balance audio output latency and power consumption.
    Balanced,
    /// Provide the lowest audio output latency possible without glitching.
    Interactive,
    /// Prioritize sustained playback without interruption over audio output latency.
    /// Lowest power consumption.
    Playback,
}

/// User-specified options for an audio context.
pub struct AudioGraphOptions {
    /// Number of samples that will play in one second, measured in Hz.
    pub sample_rate: f32,
    /// Type of playback.
    pub latency_hint: LatencyCategory,
}

impl Default for AudioGraphOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48000.,
            latency_hint: LatencyCategory::Interactive,
        }
    }
}

/// Representation of an audio context on the control thread.
pub struct AudioGraph {
    /// Rendering thread communication channel.
    sender: Sender<AudioRenderThreadMsg>,
    /// State of the audio context on the control thread.
    state: ProcessingState,
    /// Number of samples that will be played in one second.
    sample_rate: f32,
    /// The identifier of an AudioDestinationNode with a single input
    /// representing the final destination for all audio.
    dest_node: NodeId,
}

impl AudioGraph {
    /// Constructs a new audio context.
    pub fn new(options: Option<AudioGraphOptions>) -> Self {
        let options = options.unwrap_or_default();
        let sample_rate = options.sample_rate;

        let (sender, receiver) = mpsc::channel();
        let sender_ = sender.clone();
        let graph_impl = GraphImpl::new();
        let dest_node = graph_impl.dest_id();
        Builder::new()
            .name("AudioRenderThread".to_owned())
            .spawn(move || {
                AudioRenderThread::start(receiver, sender_, options.sample_rate, graph_impl)
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
    pub fn decode_audio_data(&self, data: Vec<u8>, callbacks: AudioDecoderCallbacks) {
        let mut options = AudioDecoderOptions::default();
        options.sample_rate = self.sample_rate;
        Builder::new()
            .name("AudioDecoder".to_owned())
            .spawn(move || {
                #[cfg(feature = "gst")]
                let audio_decoder = GStreamerAudioDecoder::new();

                audio_decoder.decode(data, callbacks, Some(options));
            })
            .unwrap();
    }
}

impl Drop for AudioGraph {
    fn drop(&mut self) {
        let _ = self.sender.send(AudioRenderThreadMsg::Close);
    }
}
