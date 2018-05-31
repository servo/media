use audio::gain_node::GainNodeOptions;
use audio::oscillator_node::OscillatorNodeOptions;
use audio::block::Chunk;

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

pub struct BlockInfo {
    pub sample_rate: f32,
    pub frame: u32,
    pub time: f64,
}

pub trait AudioNodeEngine: Send {
    // XXX Create an AudioBuffer abstraction
    fn process(
        &mut self,
        inputs: Chunk,
        info: &BlockInfo,
    ) -> Chunk;

    fn message(&mut self, _: AudioNodeMessage) {

    }
}


pub enum AudioNodeMessage {
    SetFloatParam(f32)
}
