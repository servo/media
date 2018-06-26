use audio::channel_node::ChannelNodeOptions;
use audio::block::{Chunk, Tick};
use audio::buffer_source_node::{AudioBufferSourceNodeMessage, AudioBufferSourceNodeOptions};
use audio::gain_node::{GainNodeMessage, GainNodeOptions};
use audio::oscillator_node::{OscillatorNodeMessage, OscillatorNodeOptions};
use std::sync::mpsc::Sender;

/// Type of AudioNodeEngine.
pub enum AudioNodeType {
    AnalyserNode,
    BiquadFilterNode,
    AudioBuffer,
    AudioBufferSourceNode(AudioBufferSourceNodeOptions),
    ChannelMergerNode(ChannelNodeOptions),
    ChannelSplitterNode,
    ConstantSourceNode,
    ConvolverNode,
    DelayNode,
    DestinationNode,
    DynamicsCompressionNode,
    GainNode(GainNodeOptions),
    IIRFilterNode,
    OscillatorNode(OscillatorNodeOptions),
    PannerNode,
    PeriodicWave,
    ScriptProcessorNode,
    StereoPannerNode,
    WaveShaperNode,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ChannelCountMode {
    Max,
    ClampedMax,
    Explicit
}


#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ChannelInterpretation {
    Discrete,
    Speakers
}

#[derive(Copy, Clone)]
pub struct BlockInfo {
    pub sample_rate: f32,
    pub frame: Tick,
    pub time: f64,
}

impl BlockInfo {
    /// Given the current block, calculate the absolute zero-relative
    /// tick of the given tick
    pub fn absolute_tick(&self, tick: Tick) -> Tick {
        self.frame + tick
    }
}

/// This trait represents the common features of all audio nodes.
pub trait AudioNodeEngine: Send {
    fn process(&mut self, inputs: Chunk, info: &BlockInfo) -> Chunk;

    fn message(&mut self, _: AudioNodeMessage, _sample_rate: f32) {}

    fn input_count(&self) -> u32 {
        1
    }
    fn output_count(&self) -> u32 {
        1
    }

    /// Number of input channels for each input port
    fn channel_count(&self) -> u8 {
        1
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Max
    }

    fn channel_interpretation(&self) -> ChannelInterpretation {
        ChannelInterpretation::Speakers
    }

    /// If we're the destination node, extract the contained data
    fn destination_data(&mut self) -> Option<Chunk> {
        None
    }
}

pub enum AudioNodeMessage {
    AudioBufferSourceNode(AudioBufferSourceNodeMessage),
    AudioScheduledSourceNode(AudioScheduledSourceNodeMessage),
    GainNode(GainNodeMessage),
    GetChannelCount(Sender<u8>),
    GetInputCount(Sender<u32>),
    GetOutputCount(Sender<u32>),
    OscillatorNode(OscillatorNodeMessage),
}

/// This trait represents the common features of the source nodes such as
/// AudioBufferSourceNode, ConstantSourceNode and OscillatorNode.
/// https://webaudio.github.io/web-audio-api/#AudioScheduledSourceNode
pub trait AudioScheduledSourceNode {
    /// Schedules a sound to playback at an exact time.
    /// Returns true if the scheduling request is processed succesfully.
    fn start(&mut self, tick: Tick) -> bool;
    /// Schedules a sound to stop playback at an exact time.
    /// Returns true if the scheduling request is processed successfully.
    fn stop(&mut self, tick: Tick) -> bool;
}

pub enum AudioScheduledSourceNodeMessage {
    /// Schedules a sound to playback at an exact time.
    Start(f64),
    /// Schedules a sound to stop playback at an exact time.
    Stop(f64),
}
