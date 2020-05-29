use block::Chunk;
use node::{AudioNodeEngine, BlockInfo};
use node::{AudioNodeType, ChannelInfo};
use servo_media_streams::MediaStreamId;
use sink::AudioSink;
use std::sync::mpsc::Sender;

#[derive(AudioNodeCommon)]
pub(crate) struct MediaStreamDestinationNode {
    channel_info: ChannelInfo,
    sink: Box<dyn AudioSink + 'static>,
}

impl MediaStreamDestinationNode {
    pub fn new(
        tx: Sender<MediaStreamId>,
        sample_rate: f32,
        sink: Box<dyn AudioSink + 'static>,
        channel_info: ChannelInfo,
    ) -> Self {
        let id = sink.init_stream(channel_info.count, sample_rate).expect("init_stream failed");
        sink.play().expect("Sink didn't start");
        tx.send(id).expect("Sending media stream failed");
        MediaStreamDestinationNode {
            channel_info,
            sink,
        }
    }
}

impl AudioNodeEngine for MediaStreamDestinationNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::MediaStreamDestinationNode
    }

    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        self.sink
            .push_data(inputs)
            .expect("Pushing to stream failed");
        Chunk::default()
    }

    fn output_count(&self) -> u32 {
        0
    }
}
