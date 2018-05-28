use audio::block::Chunk;
use audio::block::Tick;
use audio::gain_node::{GainNodeMessage, GainNodeOptions};
use audio::oscillator_node::{OscillatorNodeMessage, OscillatorNodeOptions};

/// Type of AudioNodeEngine.
pub enum AudioNodeType {
    AnalyserNode,
    BiquadFilterNode,
    AudioBuffer,
    AudioBufferSourceNode,
    ChannelMergerNode,
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
    // XXX Create an AudioBuffer abstraction
    fn process(&mut self, inputs: Chunk, info: &BlockInfo) -> Chunk;

    fn message(&mut self, _: AudioNodeMessage, _sample_rate: f32) {}
}

/// Current state of an AudioScheduledSourceNode.
pub enum AudioNodeMessage {
    OscillatorNode(OscillatorNodeMessage),
    GainNode(GainNodeMessage),
}

/// Current state of an AudioScheduledSourceNode.
#[derive(Clone, Copy, PartialEq)]
pub enum AudioScheduledSourceNodeState {
    /// The AudioScheduledSourceNodeState is playing or will be playing back
    /// sound at the given time.
    Playing(f64),
    /// The AudioScheduledSourceNodeState is not playing or will stop playing
    /// back at the given time.
    Stopped(f64),
}

/// This trait represents the common features of the source nodes such as
/// AudioBufferSourceNode, ConstantSourceNode and OscillatorNode.
pub trait AudioScheduledSourceNode {
    /// Schedules a sound to playback at an exact time.
    fn start(&self, when: f64);
    /// Schedules a sound to stop playback at an exact time.
    fn stop(&self, when: f64);
}
