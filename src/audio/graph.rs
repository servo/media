use audio::graph_thread::{AudioGraphMsg, AudioGraphThread};
use std::sync::mpsc::{self, Sender};
use std::thread::Builder;

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

    pub fn resume_processing(&self) {
        let _ = self.sender.send(AudioGraphMsg::ResumeProcessing);
    }

    pub fn pause_processing(&self) {
        let _ = self.sender.send(AudioGraphMsg::PauseProcessing);
    }
}
