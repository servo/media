use audio::graph::{AudioGraphProxy, AudioGraphProxyMsg};
use std::collections::HashMap;
use std::sync::mpsc::Receiver;

pub enum MediaThreadMsg {
    CreateAudioGraph(usize),
    AudioGraphRequest(usize, AudioGraphProxyMsg),
}

pub fn on_audio_graph_request(audio_graph: &AudioGraphProxy, msg: AudioGraphProxyMsg) {
    match msg {
        AudioGraphProxyMsg::CreateNode(node_type, sender) => {
            let node_id = audio_graph.create_node(node_type);
            let _ = sender.send(node_id);
        }
        AudioGraphProxyMsg::Resume => {
            audio_graph.resume_processing();
        }
        AudioGraphProxyMsg::Pause => {
            audio_graph.pause_processing();
        }
        AudioGraphProxyMsg::MessageNode(node_id, node_type) => {
            audio_graph.message_node(node_id, node_type)
        }
    };
}

pub fn event_loop(event_queue: Receiver<MediaThreadMsg>) {
    let mut audio_graphs = HashMap::new();
    loop {
        match event_queue.recv().unwrap() {
            MediaThreadMsg::CreateAudioGraph(id) => {
                audio_graphs.insert(id, AudioGraphProxy::new());
            }
            MediaThreadMsg::AudioGraphRequest(id, msg) => {
                if let Some(audio_graph) = audio_graphs.get(&id) {
                    on_audio_graph_request(audio_graph, msg);
                } else {
                    debug_assert!(false, "Audio graph ID not found");
                }
            }
        }
    }
}
