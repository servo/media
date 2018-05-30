use audio::node::{AudioNodeMessage, AudioNodeType};
use audio::render_thread::AudioRenderThread;
use audio::render_thread::{AudioRenderThreadMsg, AudioRenderThreadSyncMsg};
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread::Builder;

static NEXT_NODE_ID: AtomicUsize = ATOMIC_USIZE_INIT;

#[derive(Debug, PartialEq)]
pub enum ProcessingState {
    Suspended,
    Running,
    Closed,
}

pub struct AudioGraph {
    sender: Sender<AudioRenderThreadMsg>,
    sync_sender: SyncSender<AudioRenderThreadSyncMsg>,
    state: ProcessingState,
    sample_rate: f32,
}

impl AudioGraph {
    pub fn new() -> Self {
        // XXX Get this from AudioContextOptions.
        let sample_rate = 44100.;

        let (sender, receiver) = mpsc::channel();
        let sender_ = sender.clone();
        let (sync_sender, sync_receiver) = mpsc::sync_channel(0);
        Builder::new()
            .name("AudioRenderThread".to_owned())
            .spawn(move || {
                AudioRenderThread::start(receiver, sync_receiver, sender_, sample_rate)
                    .expect("Could not start AudioRenderThread");
            })
            .unwrap();
        Self {
            sender,
            sync_sender,
            state: ProcessingState::Suspended,
            sample_rate,
        }
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn current_time(&self) -> f64 {
        let (sender, receiver) = mpsc::channel();
        let _ = self.sync_sender
            .send(AudioRenderThreadSyncMsg::GetCurrentTime(sender));
        receiver.recv().unwrap()
    }

    pub fn create_node(&self, node_type: AudioNodeType) -> usize {
        let node_id = NEXT_NODE_ID.fetch_add(1, Ordering::SeqCst);
        let _ = self.sender
            .send(AudioRenderThreadMsg::CreateNode(node_type));
        node_id
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

    pub fn message_node(&self, id: usize, msg: AudioNodeMessage) {
        let _ = self.sender.send(AudioRenderThreadMsg::MessageNode(id, msg));
    }
}
