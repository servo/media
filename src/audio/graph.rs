use audio::node::{AudioNodeMessage, AudioNodeType};
use audio::render_thread::{AudioRenderThread, AudioRenderThreadMsg};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

static NEXT_NODE_ID: AtomicUsize = ATOMIC_USIZE_INIT;

pub struct AudioGraph {
    sender: Sender<AudioRenderThreadMsg>,
}

impl AudioGraph {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let sender_ = sender.clone();
        Builder::new()
            .name("AudioRenderThread".to_owned())
            .spawn(move || {
                AudioRenderThread::start(receiver, sender_)
                    .expect("Could not start AudioRenderThread");
            })
            .unwrap();
        Self { sender }
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> usize {
        let node_id = NEXT_NODE_ID.fetch_add(1, Ordering::SeqCst);
        let _ = self.sender
            .send(AudioRenderThreadMsg::CreateNode(node_type));
        node_id
    }

    pub fn resume_processing(&self) {
        let _ = self.sender.send(AudioRenderThreadMsg::ResumeProcessing);
    }

    pub fn pause_processing(&self) {
        let _ = self.sender.send(AudioRenderThreadMsg::PauseProcessing);
    }

    pub fn message_node(&self, id: usize, msg: AudioNodeMessage) {
        let _ = self.sender.send(AudioRenderThreadMsg::MessageNode(id, msg));
    }
}
