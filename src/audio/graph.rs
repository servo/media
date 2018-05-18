use audio::graph_thread::{AudioGraphMsg, AudioGraphThread};
use audio::node::AudioNodeType;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

static NEXT_NODE_ID: AtomicUsize = ATOMIC_USIZE_INIT;

pub struct AudioGraph {
    sender: Sender<AudioGraphMsg>,
}

impl AudioGraph {
    pub fn new() -> Result<Self, ()> {
        let (sender, receiver) = mpsc::channel();
        Builder::new()
            .name("AudioGraph".to_owned())
            .spawn(move || {
                AudioGraphThread::start(receiver);
            })
            .unwrap();
        Ok(Self { sender })
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> usize {
        let node_id = NEXT_NODE_ID.fetch_add(1, Ordering::SeqCst);
        let _ = self.sender
            .send(AudioGraphMsg::CreateNode(node_id, node_type));
        node_id
    }

    pub fn resume_processing(&self) {
        let _ = self.sender.send(AudioGraphMsg::ResumeProcessing);
    }

    pub fn pause_processing(&self) {
        let _ = self.sender.send(AudioGraphMsg::PauseProcessing);
    }
}
