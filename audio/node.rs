use biquad_filter_node::{BiquadFilterNodeMessage, BiquadFilterNodeOptions};
use block::{Block, Chunk, Tick};
use boxfnonce::SendBoxFnOnce;
use buffer_source_node::{AudioBufferSourceNodeMessage, AudioBufferSourceNodeOptions};
use channel_node::ChannelNodeOptions;
use constant_source_node::ConstantSourceNodeOptions;
use gain_node::GainNodeOptions;
use oscillator_node::{OscillatorNodeMessage, OscillatorNodeOptions};
use panner_node::{PannerNodeMessage, PannerNodeOptions};
use param::{Param, ParamRate, ParamType, UserAutomationEvent};
use stereo_panner::StereoPannerOptions;
use std::sync::mpsc::Sender;

/// Information required to construct an audio node
pub enum AudioNodeInit {
    AnalyserNode(Box<dyn FnMut(Block) + Send>),
    BiquadFilterNode(BiquadFilterNodeOptions),
    AudioBuffer,
    AudioBufferSourceNode(AudioBufferSourceNodeOptions),
    ChannelMergerNode(ChannelNodeOptions),
    ChannelSplitterNode,
    ConstantSourceNode(ConstantSourceNodeOptions),
    ConvolverNode,
    DelayNode,
    DynamicsCompressionNode,
    GainNode(GainNodeOptions),
    IIRFilterNode,
    OscillatorNode(OscillatorNodeOptions),
    PannerNode(PannerNodeOptions),
    PeriodicWave,
    ScriptProcessorNode,
    StereoPannerNode(StereoPannerOptions),
    WaveShaperNode,
}

/// Type of AudioNodeEngine.
#[derive(Debug, Clone, Copy)]
pub enum AudioNodeType {
    /// Not a constructable node
    AudioListenerNode,
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
    GainNode,
    IIRFilterNode,
    OscillatorNode,
    PannerNode,
    PeriodicWave,
    ScriptProcessorNode,
    StereoPannerNode,
    WaveShaperNode,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ChannelCountMode {
    Max,
    ClampedMax,
    Explicit,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ChannelInterpretation {
    Discrete,
    Speakers,
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

pub struct ChannelInfo {
    pub count: u8,
    pub mode: ChannelCountMode,
    pub interpretation: ChannelInterpretation,
}

impl Default for ChannelInfo {
    fn default() -> Self {
        ChannelInfo {
            count: 2,
            mode: ChannelCountMode::Max,
            interpretation: ChannelInterpretation::Speakers,
        }
    }
}

pub(crate) trait AudioNodeCommon {
    fn channel_info(&self) -> &ChannelInfo;

    fn channel_info_mut(&mut self) -> &mut ChannelInfo;
}

/// This trait represents the common features of all audio nodes.
pub(crate) trait AudioNodeEngine: Send + AudioNodeCommon {
    fn node_type(&self) -> AudioNodeType;

    fn process(&mut self, inputs: Chunk, info: &BlockInfo) -> Chunk;

    fn message(&mut self, msg: AudioNodeMessage, sample_rate: f32) {
        match msg {
            AudioNodeMessage::GetParamValue(id, tx) => {
                let _ = tx.send(self.get_param(id).value());
            }
            AudioNodeMessage::SetChannelCount(c) => self.set_channel_count(c),
            AudioNodeMessage::SetChannelMode(c) => self.set_channel_count_mode(c),
            AudioNodeMessage::SetChannelInterpretation(c) => self.set_channel_interpretation(c),
            AudioNodeMessage::SetParam(id, event) => {
                self.get_param(id).insert_event(event.to_event(sample_rate))
            }
            AudioNodeMessage::SetParamRate(id, rate) => self.get_param(id).set_rate(rate),
            _ => self.message_specific(msg, sample_rate),
        }
    }

    /// Messages specific to this node
    fn message_specific(&mut self, _: AudioNodeMessage, _sample_rate: f32) {}

    fn input_count(&self) -> u32 {
        1
    }
    fn output_count(&self) -> u32 {
        1
    }

    /// Number of input channels for each input port
    fn channel_count(&self) -> u8 {
        self.channel_info().count
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        self.channel_info().mode
    }

    fn channel_interpretation(&self) -> ChannelInterpretation {
        self.channel_info().interpretation
    }

    fn set_channel_interpretation(&mut self, i: ChannelInterpretation) {
        self.channel_info_mut().interpretation = i
    }
    fn set_channel_count(&mut self, c: u8) {
        self.channel_info_mut().count = c;
    }
    fn set_channel_count_mode(&mut self, m: ChannelCountMode) {
        self.channel_info_mut().mode = m;
    }

    /// If we're the destination node, extract the contained data
    fn destination_data(&mut self) -> Option<Chunk> {
        None
    }

    fn get_param(&mut self, _: ParamType) -> &mut Param {
        panic!("No params on node {:?}", self.node_type())
    }

    fn set_listenerdata(&mut self, _: Block) {
        panic!("{:?} can't accept listener connections")
    }
}

pub enum AudioNodeMessage {
    AudioBufferSourceNode(AudioBufferSourceNodeMessage),
    AudioScheduledSourceNode(AudioScheduledSourceNodeMessage),
    BiquadFilterNode(BiquadFilterNodeMessage),
    OscillatorNode(OscillatorNodeMessage),
    PannerNode(PannerNodeMessage),
    GetParamValue(ParamType, Sender<f32>),
    SetChannelCount(u8),
    SetChannelMode(ChannelCountMode),
    SetChannelInterpretation(ChannelInterpretation),
    SetParam(ParamType, UserAutomationEvent),
    SetParamRate(ParamType, ParamRate),
}

pub struct OnEndedCallback(pub SendBoxFnOnce<'static, ()>);

impl OnEndedCallback {
    pub fn new<F: FnOnce() + Send + 'static>(callback: F) -> Self {
        OnEndedCallback(SendBoxFnOnce::new(callback))
    }
}

/// Type of message directed to AudioScheduledSourceNodes.
pub enum AudioScheduledSourceNodeMessage {
    /// Schedules a sound to playback at an exact time.
    Start(f64),
    /// Schedules a sound to stop playback at an exact time.
    Stop(f64),
    /// Register onended event callback.
    RegisterOnEndedCallback(OnEndedCallback),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShouldPlay {
    /// Don't play anything
    No,
    /// Play, given start and end tick offsets
    Between(Tick, Tick),
}
