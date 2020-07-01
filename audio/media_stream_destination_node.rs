use block::Chunk;
use node::{AudioNodeEngine, BlockInfo};
use node::{AudioNodeType, ChannelInfo};
use servo_media_streams::MediaSocket;
use sink::AudioSink;

#[derive(AudioNodeCommon)]
pub(crate) struct MediaStreamDestinationNode {
    channel_info: ChannelInfo,
    sink: Option<Box<dyn AudioSink + 'static>>,
}

impl MediaStreamDestinationNode {
    pub fn new(channel_info: ChannelInfo) -> Self {
        MediaStreamDestinationNode {
            channel_info,
            sink: None,
        }
    }
}

impl AudioNodeEngine for MediaStreamDestinationNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::MediaStreamDestinationNode
    }

    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        self.sink
            .as_ref()
            .map(|s| s.push_data(inputs).expect("Pushing to stream failed"));
        Chunk::default()
    }

    fn output_count(&self) -> u32 {
        0
    }

    fn set_socket(
        &mut self,
        sink: Box<dyn AudioSink + 'static>,
        socket: Box<dyn MediaSocket>,
        sample_rate: f32,
    ) {
        if let Some(sink) = self.sink.take() {
            let _ = sink.stop();
        }
        sink.init_stream(self.channel_info.count, sample_rate, socket)
            .expect("init_stream failed");
        sink.play().expect("Sink didn't start");
        self.sink = Some(sink);
    }
}
