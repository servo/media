use audio::block::Tick;
use audio::block::{Chunk, FRAMES_PER_BLOCK};
use audio::destination_node::DestinationNode;
use audio::gain_node::GainNode;
use audio::graph::ProcessingState;
use audio::node::BlockInfo;
use audio::node::{AudioNodeEngine, AudioNodeMessage, AudioNodeType};
use audio::oscillator_node::OscillatorNode;
use std::sync::mpsc::{Receiver, Sender};

pub enum AudioRenderThreadMsg {
    CreateNode(AudioNodeType),
    MessageNode(usize, AudioNodeMessage),
    Resume,
    Suspend,
    Close,
    SinkNeedData,
    GetCurrentTime(Sender<f64>),
}

pub struct AudioRenderThread {
    // XXX Test with a Vec for now. This should end up
    // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
    pub nodes: Vec<Box<AudioNodeEngine>>,
    /// Destination node instance.
    pub destination: DestinationNode,
    pub state: ProcessingState,
    pub sample_rate: f32,
    pub current_time: f64,
    pub current_frame: Tick,
}

impl AudioRenderThread {
    pub fn start(
        event_queue: Receiver<AudioRenderThreadMsg>,
        sender: Sender<AudioRenderThreadMsg>,
        sample_rate: f32,
        ) -> Result<(), ()> {
        let destination = DestinationNode::new()?;
        destination.init(sample_rate, sender)?;
        let mut graph = Self {
            // XXX Test with a vec map for now. This should end up
            // being a graph, like https://docs.rs/petgraph/0.4.12/petgraph/.
            nodes: Vec::new(),
            destination,
            state: ProcessingState::Suspended,
            sample_rate,
            current_time: 0.,
            current_frame: Tick(0),
        };

        graph.event_loop(event_queue);

        Ok(())
    }

    fn resume(&mut self) {
        if self.state == ProcessingState::Running {
            return;
        }
        self.state = ProcessingState::Running;
    }

    fn suspend(&mut self) {
        if self.state == ProcessingState::Suspended {
            return;
        }
        self.state = ProcessingState::Suspended;
    }

    fn close(&mut self) {
        if self.state == ProcessingState::Closed {
            return;
        }
        self.state = ProcessingState::Closed;
    }

    fn create_node(&mut self, node_type: AudioNodeType) {
        let node: Box<AudioNodeEngine> = match node_type {
            AudioNodeType::OscillatorNode(options) => Box::new(OscillatorNode::new(options)),
            AudioNodeType::GainNode(options) => Box::new(GainNode::new(options)),
            // We don't allow direct creation of DestinationNodes.
            AudioNodeType::DestinationNode => unreachable!(),
            _ => unimplemented!(),
        };
        self.nodes.push(node);
    }

    fn process(&mut self) {
        let mut data = Chunk::default();
        let info = BlockInfo {
            sample_rate: self.sample_rate,
            frame: self.current_frame,
            time: self.current_time,
        };
        for node in self.nodes.iter_mut() {
            data = node.process(data, &info).unwrap();
        }
        self.destination.process(data, &info);
    }

    fn event_loop(&mut self, event_queue: Receiver<AudioRenderThreadMsg>) {
        let sample_rate = self.sample_rate;
        let handle_msg = move |context: &mut Self, msg: AudioRenderThreadMsg| -> bool {
            let mut break_loop = false;
            match msg {
                AudioRenderThreadMsg::CreateNode(node_type) => {
                    context.create_node(node_type);
                }
                AudioRenderThreadMsg::Resume => {
                    context.resume();
                }
                AudioRenderThreadMsg::Suspend => {
                    context.suspend();
                }
                AudioRenderThreadMsg::Close => {
                    context.close();
                    break_loop = true;
                }
                AudioRenderThreadMsg::GetCurrentTime(response) => {
                    response.send(context.current_time).unwrap()
                }
                AudioRenderThreadMsg::MessageNode(index, msg) => {
                    context.nodes[index].message(msg, sample_rate)
                }
                AudioRenderThreadMsg::SinkNeedData => {
                    // Do nothing. This will simply unblock the thread so we
                    // can restart the non-blocking event loop.
                }
            };

            break_loop
        };

        loop {
            if self.destination.has_enough_data() || self.state == ProcessingState::Suspended {
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

                // Process the nodes of the graph.
                self.process();
                // Increment current frame by the render quantum size.
                self.current_frame += FRAMES_PER_BLOCK;
                self.current_time = self.current_frame / self.sample_rate as f64;
            }
        }
    }
}
