use crate::AudioStreamReader;
use block::{Chunk, Tick};
use node::{AudioNodeEngine, AudioScheduledSourceNodeMessage, BlockInfo, OnEndedCallback};
use node::{AudioNodeType, ChannelInfo, ShouldPlay};
use param::{Param, ParamType};

#[derive(AudioScheduledSourceNode, AudioNodeCommon)]
pub(crate) struct MediaStreamSourceNode {
    channel_info: ChannelInfo,
    /// Time at which the source should start playing.
    start_at: Option<Tick>,
    /// Time at which the source should stop playing.
    stop_at: Option<Tick>,
    /// The ended event callback.
    onended_callback: Option<OnEndedCallback>,
    reader: Box<dyn AudioStreamReader + Send>,
    playing: bool,
}

impl MediaStreamSourceNode {
    pub fn new(reader: Box<dyn AudioStreamReader + Send>, channel_info: ChannelInfo) -> Self {
        Self {
            channel_info,
            start_at: None,
            stop_at: None,
            onended_callback: None,
            reader,
            playing: false,
        }
    }
}

impl AudioNodeEngine for MediaStreamSourceNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::MediaStreamSourceNode
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 0);
        match self.should_play_at(info.frame) {
            ShouldPlay::No => {
                if self.playing {
                    self.playing = false;
                    self.reader.stop();
                }
                inputs.blocks.push(Default::default());
                return inputs;
            }
            ShouldPlay::Between(_start, _end) => (),
        };

        if !self.playing {
            self.playing = true;
            self.reader.start();
        }

        let block = self.reader.pull();
        // XXXManishearth truncate start and end
        inputs.blocks.push(block);

        inputs
    }

    fn input_count(&self) -> u32 {
        0
    }

    fn get_param(&mut self, _: ParamType) -> &mut Param {
        panic!("No params on MediaStreamSourceNode");
    }
    make_message_handler!(AudioScheduledSourceNode: handle_source_node_message);
}
