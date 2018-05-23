use audio::graph_thread::{AudioGraphThread, AudioGraphThreadMsg};
use audio::node::AudioNodeType;
use media_thread::MediaThreadMsg;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

static NEXT_NODE_ID: AtomicUsize = ATOMIC_USIZE_INIT;

pub struct AudioGraph {
    id: usize,
    sender: Sender<MediaThreadMsg>,
}

impl AudioGraph {
    pub fn new(id: usize, sender: Sender<MediaThreadMsg>) -> Self {
        let _ = sender.send(MediaThreadMsg::CreateAudioGraph(id));
        Self { id, sender }
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> usize {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .send(MediaThreadMsg::AudioGraphRequest(
                self.id,
                AudioGraphProxyMsg::CreateNode(node_type, sender),
            ))
            .unwrap();
        receiver.recv().unwrap()
    }

    pub fn resume_processing(&self) {
        self.sender
            .send(MediaThreadMsg::AudioGraphRequest(
                self.id,
                AudioGraphProxyMsg::Resume,
            ))
            .unwrap();
    }

    pub fn pause_processing(&self) {
        self.sender
            .send(MediaThreadMsg::AudioGraphRequest(
                self.id,
                AudioGraphProxyMsg::Pause,
            ))
            .unwrap();
    }
}

pub enum AudioGraphProxyMsg {
    CreateNode(AudioNodeType, Sender<usize>),
    Resume,
    Pause,
}

pub struct AudioGraphProxy {
    sender: Sender<AudioGraphThreadMsg>,
}

impl AudioGraphProxy {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Builder::new()
            .name("AudioGraph".to_owned())
            .spawn(move || {
                AudioGraphThread::start(receiver);
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
}
