use audio::graph_thread::{AudioGraphThread, AudioGraphThreadMsg};
use audio::node::{AudioNodeType, AudioNodeMessage};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

static NEXT_NODE_ID: AtomicUsize = ATOMIC_USIZE_INIT;

pub struct AudioGraph {
    sender: Sender<AudioGraphThreadMsg>,
}

impl AudioGraph {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let s2 = sender.clone();
        Builder::new()
            .name("AudioGraph".to_owned())
            .spawn(move || {
                AudioGraphThread::start(s2, receiver);
            })
            .unwrap();
        Self { sender }
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> usize {
        let node_id = NEXT_NODE_ID.fetch_add(1, Ordering::SeqCst);
        let _ = self.sender
            .send(AudioGraphThreadMsg::CreateNode(node_type));
        node_id
    }

    pub fn resume_processing(&self) {
        let _ = self.sender.send(AudioGraphThreadMsg::ResumeProcessing);
    }

    pub fn pause_processing(&self) {
        let _ = self.sender.send(AudioGraphThreadMsg::PauseProcessing);
    }

    pub fn message_node(&self, id: usize, msg: AudioNodeMessage) {
        let _ = self.sender.send(AudioGraphThreadMsg::MessageNode(id, msg));
    }
}
