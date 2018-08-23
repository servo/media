use block::{Chunk, Tick, FRAMES_PER_BLOCK};
use buffer_source_node::AudioBufferSourceNode;
use channel_node::{ChannelMergerNode, ChannelSplitterNode};
use context::{AudioContextOptions, ProcessingState, StateChangeResult};
use gain_node::GainNode;
use graph::{AudioGraph, InputPort, NodeId, OutputPort, PortId};
use node::BlockInfo;
use node::{AudioNodeEngine, AudioNodeInit, AudioNodeMessage};
use offline_sink::OfflineAudioSink;
use oscillator_node::OscillatorNode;
use panner_node::PannerNode;
use sink::AudioSink;
use std::sync::mpsc::{Receiver, Sender};
use AudioBackend;

pub enum AudioRenderThreadMsg {
    CreateNode(AudioNodeInit, Sender<NodeId>),
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

    SetSinkEosCallback(Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>),
}

pub enum Sink<B: AudioBackend> {
    RealTime(B::Sink),
    Offline(OfflineAudioSink),
}

impl<B: AudioBackend> AudioSink for Sink<B> {
    fn init(&self, sample_rate: f32, sender: Sender<AudioRenderThreadMsg>) -> Result<(), ()> {
        match *self {
            Sink::RealTime(ref sink) => sink.init(sample_rate, sender),
            Sink::Offline(ref sink) => sink.init(sample_rate, sender),
        }
    }

    fn play(&self) -> Result<(), ()> {
        match *self {
            Sink::RealTime(ref sink) => sink.play(),
            Sink::Offline(ref sink) => sink.play(),
        }
    }

    fn stop(&self) -> Result<(), ()> {
        match *self {
            Sink::RealTime(ref sink) => sink.stop(),
            Sink::Offline(ref sink) => sink.stop(),
        }
    }

    fn has_enough_data(&self) -> bool {
        match *self {
            Sink::RealTime(ref sink) => sink.has_enough_data(),
            Sink::Offline(ref sink) => sink.has_enough_data(),
        }
    }

    fn push_data(&self, chunk: Chunk) -> Result<(), ()> {
        match *self {
            Sink::RealTime(ref sink) => sink.push_data(chunk),
            Sink::Offline(ref sink) => sink.push_data(chunk),
        }
    }

    fn set_eos_callback(&self, callback: Box<Fn(Box<AsRef<[f32]>>) + Send + Sync + 'static>) {
        match *self {
            Sink::RealTime(ref sink) => sink.set_eos_callback(callback),
            Sink::Offline(ref sink) => sink.set_eos_callback(callback),
        }
    }
}

pub struct AudioRenderThread<B: AudioBackend> {
    pub graph: AudioGraph,
    pub sink: Sink<B>,
    pub state: ProcessingState,
    pub sample_rate: f32,
    pub current_time: f64,
    pub current_frame: Tick,
}

impl<B: AudioBackend + 'static> AudioRenderThread<B> {
    /// Start the audio render thread
    pub fn start(
        event_queue: Receiver<AudioRenderThreadMsg>,
        sender: Sender<AudioRenderThreadMsg>,
        sample_rate: f32,
        graph: AudioGraph,
        options: AudioContextOptions,
    ) -> Result<(), ()> {
        let sink = match options {
            AudioContextOptions::RealTimeAudioContext(_) => Sink::RealTime(B::make_sink()?),
            AudioContextOptions::OfflineAudioContext(options) => Sink::Offline(
                OfflineAudioSink::new(options.channels as usize, options.length),
            ),
        };

        let mut graph = Self {
            graph,
            sink,
            state: ProcessingState::Suspended,
            sample_rate,
            current_time: 0.,
            current_frame: Tick(0),
        };

        graph.sink.init(sample_rate, sender)?;
        graph.event_loop(event_queue);

        Ok(())
    }

    make_render_thread_state_change!(resume, Running, play);

    make_render_thread_state_change!(suspend, Suspended, stop);

    fn create_node(&mut self, node_type: AudioNodeInit) -> NodeId {
        let mut needs_listener = false;
        let node: Box<AudioNodeEngine> = match node_type {
            AudioNodeInit::AudioBufferSourceNode(options) => {
                Box::new(AudioBufferSourceNode::new(options))
            }
            AudioNodeInit::GainNode(options) => Box::new(GainNode::new(options)),
            AudioNodeInit::PannerNode(options) => {
                needs_listener = true;
                Box::new(PannerNode::new(options))
            },
            AudioNodeInit::OscillatorNode(options) => Box::new(OscillatorNode::new(options)),
            AudioNodeInit::ChannelMergerNode(options) => Box::new(ChannelMergerNode::new(options)),
            AudioNodeInit::ChannelSplitterNode(options) => {
                Box::new(ChannelSplitterNode::new(options))
            }
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
        let info = BlockInfo {
            sample_rate: self.sample_rate,
            frame: self.current_frame,
            time: self.current_time,
        };
        self.graph.process(&info)
    }

    fn event_loop(&mut self, event_queue: Receiver<AudioRenderThreadMsg>) {
        let sample_rate = self.sample_rate;
        let handle_msg = move |context: &mut Self, msg: AudioRenderThreadMsg| -> bool {
            let mut break_loop = false;
            match msg {
                AudioRenderThreadMsg::CreateNode(node_type, tx) => {
                    let _ = tx.send(context.create_node(node_type));
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
