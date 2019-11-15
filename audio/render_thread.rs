use analyser_node::AnalyserNode;
use biquad_filter_node::BiquadFilterNode;
use block::{Chunk, Tick, FRAMES_PER_BLOCK};
use buffer_source_node::AudioBufferSourceNode;
use channel_node::{ChannelMergerNode, ChannelSplitterNode};
use constant_source_node::ConstantSourceNode;
use context::{AudioContextOptions, ProcessingState, StateChangeResult};
use gain_node::GainNode;
use graph::{AudioGraph, InputPort, NodeId, OutputPort, PortId};
use media_element_source_node::MediaElementSourceNode;
use node::{AudioNodeEngine, AudioNodeInit, AudioNodeMessage};
use node::{BlockInfo, ChannelInfo};
use offline_sink::OfflineAudioSink;
use oscillator_node::OscillatorNode;
use panner_node::PannerNode;
use sink::{AudioSink, AudioSinkError};
use std::sync::mpsc::{Receiver, Sender};
use stereo_panner::StereoPannerNode;
use wave_shaper_node::WaveShaperNode;

pub enum AudioRenderThreadMsg {
    CreateNode(AudioNodeInit, Sender<NodeId>, ChannelInfo),
    ConnectPorts(PortId<OutputPort>, PortId<InputPort>),
    MessageNode(NodeId, AudioNodeMessage),
    Resume(Sender<StateChangeResult>),
    Suspend(Sender<StateChangeResult>),
    Close(Sender<StateChangeResult>),
    SinkNeedData,
    GetCurrentTime(Sender<f64>),

    DisconnectAllFrom(NodeId),
    DisconnectOutput(PortId<OutputPort>),
    DisconnectBetween(NodeId, NodeId),
    DisconnectTo(NodeId, PortId<InputPort>),
    DisconnectOutputBetween(PortId<OutputPort>, NodeId),
    DisconnectOutputBetweenTo(PortId<OutputPort>, PortId<InputPort>),

    SetSinkEosCallback(Box<dyn Fn(Box<dyn AsRef<[f32]>>) + Send + Sync + 'static>),

    SetMute(bool),
}

pub enum Sink {
    RealTime(Box<dyn AudioSink>),
    Offline(OfflineAudioSink),
}

impl AudioSink for Sink {
    fn init(
        &self,
        sample_rate: f32,
        sender: Sender<AudioRenderThreadMsg>,
    ) -> Result<(), AudioSinkError> {
        match *self {
            Sink::RealTime(ref sink) => sink.init(sample_rate, sender),
            Sink::Offline(ref sink) => Ok(sink.init(sample_rate, sender).unwrap()),
        }
    }

    fn play(&self) -> Result<(), AudioSinkError> {
        match *self {
            Sink::RealTime(ref sink) => sink.play(),
            Sink::Offline(ref sink) => Ok(sink.play().unwrap()),
        }
    }

    fn stop(&self) -> Result<(), AudioSinkError> {
        match *self {
            Sink::RealTime(ref sink) => sink.stop(),
            Sink::Offline(ref sink) => Ok(sink.stop().unwrap()),
        }
    }

    fn has_enough_data(&self) -> bool {
        match *self {
            Sink::RealTime(ref sink) => sink.has_enough_data(),
            Sink::Offline(ref sink) => sink.has_enough_data(),
        }
    }

    fn push_data(&self, chunk: Chunk) -> Result<(), AudioSinkError> {
        match *self {
            Sink::RealTime(ref sink) => sink.push_data(chunk),
            Sink::Offline(ref sink) => Ok(sink.push_data(chunk).unwrap()),
        }
    }

    fn set_eos_callback(
        &self,
        callback: Box<dyn Fn(Box<dyn AsRef<[f32]>>) + Send + Sync + 'static>,
    ) {
        match *self {
            Sink::RealTime(ref sink) => sink.set_eos_callback(callback),
            Sink::Offline(ref sink) => sink.set_eos_callback(callback),
        }
    }
}

pub struct AudioRenderThread {
    pub graph: AudioGraph,
    pub sink: Sink,
    pub state: ProcessingState,
    pub sample_rate: f32,
    pub current_time: f64,
    pub current_frame: Tick,
    pub muted: bool,
}

impl AudioRenderThread {
    /// Initializes the AudioRenderThread object
    ///
    /// You must call .event_loop() on this to run it!
    fn prepare_thread<F>(
        make_sink: F,
        sender: Sender<AudioRenderThreadMsg>,
        sample_rate: f32,
        graph: AudioGraph,
        options: AudioContextOptions,
    ) -> Result<Self, AudioSinkError>
    where
        F: FnOnce() -> Result<Box<dyn AudioSink + 'static>, AudioSinkError>,
    {
        let sink = match options {
            AudioContextOptions::RealTimeAudioContext(_) => Sink::RealTime(make_sink()?),
            AudioContextOptions::OfflineAudioContext(options) => Sink::Offline(
                OfflineAudioSink::new(options.channels as usize, options.length),
            ),
        };

        sink.init(sample_rate, sender)?;

        Ok(Self {
            graph,
            sink,
            state: ProcessingState::Suspended,
            sample_rate,
            current_time: 0.,
            current_frame: Tick(0),
            muted: false,
        })
    }

    /// Start the audio render thread
    ///
    /// In case something fails, it will instead start a thread with a dummy backend
    pub fn start<F>(
        make_sink: F,
        event_queue: Receiver<AudioRenderThreadMsg>,
        sender: Sender<AudioRenderThreadMsg>,
        sample_rate: f32,
        graph: AudioGraph,
        options: AudioContextOptions,
    ) where
        F: FnOnce() -> Result<Box<dyn AudioSink + 'static>, AudioSinkError>,
    {
        let mut thread =
            Self::prepare_thread(make_sink, sender.clone(), sample_rate, graph, options)
                .expect("Could not start audio render thread");
        thread.event_loop(event_queue)
    }

    make_render_thread_state_change!(resume, Running, play);

    make_render_thread_state_change!(suspend, Suspended, stop);

    fn create_node(&mut self, node_type: AudioNodeInit, ch: ChannelInfo) -> NodeId {
        let mut needs_listener = false;
        let node: Box<dyn AudioNodeEngine> = match node_type {
            AudioNodeInit::AnalyserNode(sender) => Box::new(AnalyserNode::new(sender, ch)),
            AudioNodeInit::AudioBufferSourceNode(options) => {
                Box::new(AudioBufferSourceNode::new(options, ch))
            }
            AudioNodeInit::BiquadFilterNode(options) => {
                Box::new(BiquadFilterNode::new(options, ch, self.sample_rate))
            }
            AudioNodeInit::GainNode(options) => Box::new(GainNode::new(options, ch)),
            AudioNodeInit::StereoPannerNode(options) => {
                Box::new(StereoPannerNode::new(options, ch))
            }
            AudioNodeInit::PannerNode(options) => {
                needs_listener = true;
                Box::new(PannerNode::new(options, ch))
            }
            AudioNodeInit::OscillatorNode(options) => Box::new(OscillatorNode::new(options, ch)),
            AudioNodeInit::ChannelMergerNode(options) => {
                Box::new(ChannelMergerNode::new(options, ch))
            }
            AudioNodeInit::ConstantSourceNode(options) => {
                Box::new(ConstantSourceNode::new(options, ch))
            }
            AudioNodeInit::ChannelSplitterNode => Box::new(ChannelSplitterNode::new(ch)),
            AudioNodeInit::WaveShaperNode(options) => Box::new(WaveShaperNode::new(options, ch)),
            AudioNodeInit::MediaElementSourceNode => Box::new(MediaElementSourceNode::new(ch)),
            _ => unimplemented!(),
        };
        let id = self.graph.add_node(node);
        if needs_listener {
            let listener = self.graph.listener_id().output(0);
            self.graph.add_edge(listener, id.listener());
        }
        id
    }

    fn connect_ports(&mut self, output: PortId<OutputPort>, input: PortId<InputPort>) {
        self.graph.add_edge(output, input)
    }

    fn process(&mut self) -> Chunk {
        if self.muted {
            return Chunk::explicit_silence();
        }

        let info = BlockInfo {
            sample_rate: self.sample_rate,
            frame: self.current_frame,
            time: self.current_time,
        };
        self.graph.process(&info)
    }

    fn set_mute(&mut self, val: bool) -> () {
        self.muted = val;
    }

    fn event_loop(&mut self, event_queue: Receiver<AudioRenderThreadMsg>) {
        let sample_rate = self.sample_rate;
        let handle_msg = move |context: &mut Self, msg: AudioRenderThreadMsg| -> bool {
            let mut break_loop = false;
            match msg {
                AudioRenderThreadMsg::CreateNode(node_type, tx, ch) => {
                    let _ = tx.send(context.create_node(node_type, ch));
                }
                AudioRenderThreadMsg::ConnectPorts(output, input) => {
                    context.connect_ports(output, input);
                }
                AudioRenderThreadMsg::Resume(tx) => {
                    let _ = tx.send(context.resume());
                }
                AudioRenderThreadMsg::Suspend(tx) => {
                    let _ = tx.send(context.suspend());
                }
                AudioRenderThreadMsg::Close(tx) => {
                    let _ = tx.send(context.suspend());
                    break_loop = true;
                }
                AudioRenderThreadMsg::GetCurrentTime(response) => {
                    response.send(context.current_time).unwrap()
                }
                AudioRenderThreadMsg::MessageNode(id, msg) => {
                    context.graph.node_mut(id).message(msg, sample_rate)
                }
                AudioRenderThreadMsg::SinkNeedData => {
                    // Do nothing. This will simply unblock the thread so we
                    // can restart the non-blocking event loop.
                }
                AudioRenderThreadMsg::DisconnectAllFrom(id) => {
                    context.graph.disconnect_all_from(id)
                }
                AudioRenderThreadMsg::DisconnectOutput(out) => context.graph.disconnect_output(out),
                AudioRenderThreadMsg::DisconnectBetween(from, to) => {
                    context.graph.disconnect_between(from, to)
                }
                AudioRenderThreadMsg::DisconnectTo(from, to) => {
                    context.graph.disconnect_to(from, to)
                }
                AudioRenderThreadMsg::DisconnectOutputBetween(from, to) => {
                    context.graph.disconnect_output_between(from, to)
                }
                AudioRenderThreadMsg::DisconnectOutputBetweenTo(from, to) => {
                    context.graph.disconnect_output_between_to(from, to)
                }
                AudioRenderThreadMsg::SetSinkEosCallback(callback) => {
                    context.sink.set_eos_callback(callback);
                }
                AudioRenderThreadMsg::SetMute(val) => {
                    context.set_mute(val);
                }
            };

            break_loop
        };

        loop {
            if self.sink.has_enough_data() || self.state == ProcessingState::Suspended {
                // If we are not processing audio or
                // if we have already pushed enough data into the audio sink
                // we wait for messages coming from the control thread or
                // the audio sink. The audio sink will notify whenever it
                // needs more data.
                if let Ok(msg) = event_queue.recv() {
                    if handle_msg(self, msg) {
                        break;
                    }
                }
            } else {
                // If we have not pushed enough data into the audio sink yet,
                // we process the control message queue
                if let Ok(msg) = event_queue.try_recv() {
                    if handle_msg(self, msg) {
                        break;
                    }
                }

                if self.state == ProcessingState::Suspended {
                    // Bail out if we just suspended processing.
                    continue;
                }

                // push into the audio sink the result of processing a
                // render quantum.
                let data = self.process();
                if self.sink.push_data(data).is_ok() {
                    // increment current frame by the render quantum size.
                    self.current_frame += FRAMES_PER_BLOCK;
                    self.current_time = self.current_frame / self.sample_rate as f64;
                } else {
                    eprintln!("Could not push data to audio sink");
                }
            }
        }
    }
}
