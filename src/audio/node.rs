use audio::oscillator_node::OscillatorNodeOptions;

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
    DynamicsCompressionNode,
    GainNode,
    IIRFilterNode,
    OscillatorNode(OscillatorNodeOptions),
    PannerNode,
    PeriodicWave,
    ScriptProcessorNode,
    StereoPannerNode,
    WaveShaperNode,
}

pub trait AudioNodeEngine: Send {
    // XXX Create an AudioBuffer abstraction
    fn process(
        &self,
        data: &mut [u8],
        rate: u32,
    );
}
