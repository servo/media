use block::{Block, Chunk, Tick, FRAMES_PER_BLOCK};
use node::{AudioNodeEngine, AudioScheduledSourceNodeMessage, BlockInfo, OnEndedCallback};
use node::{AudioNodeType, ChannelInfo, ShouldPlay};
use param::{Param, ParamType};

/// Control messages directed to AudioBufferSourceNodes.
#[derive(Debug, Clone)]
pub enum AudioBufferSourceNodeMessage {
    /// Set the data block holding the audio sample data to be played.
    SetBuffer(Option<AudioBuffer>),
}

/// This specifies options for constructing an AudioBufferSourceNode.
#[derive(Debug, Clone)]
pub struct AudioBufferSourceNodeOptions {
    /// The audio asset to be played.
    pub buffer: Option<AudioBuffer>,
    /// The initial value for the detune AudioParam.
    pub detune: f32,
    /// The initial value for the loop_enabled attribute.
    pub loop_enabled: bool,
    /// The initial value for the loop_end attribute.
    pub loop_end: Option<f64>,
    /// The initial value for the loop_start attribute.
    pub loop_start: Option<f64>,
    /// The initial value for the playback_rate AudioParam.
    pub playback_rate: f32,
}

impl Default for AudioBufferSourceNodeOptions {
    fn default() -> Self {
        AudioBufferSourceNodeOptions {
            buffer: None,
            detune: 0.,
            loop_enabled: false,
            loop_end: None,
            loop_start: None,
            playback_rate: 1.,
        }
    }
}

/// AudioBufferSourceNode engine.
/// https://webaudio.github.io/web-audio-api/#AudioBufferSourceNode
#[derive(AudioScheduledSourceNode, AudioNodeCommon)]
#[allow(dead_code)]
pub(crate) struct AudioBufferSourceNode {
    channel_info: ChannelInfo,
    /// A data block holding the audio sample data to be played.
    buffer: Option<AudioBuffer>,
    /// "Index" of the next buffer frame to play. "Index" is in quotes because
    /// this variable maps to a playhead position (the offset in seconds can be
    /// obtained by dividing by self.buffer.sample_rate), and therefore has
    /// subsample accuracy; a fractional "index" means interpolation is needed.
    buffer_pos: f64,
    /// AudioParam to modulate the speed at which is rendered the audio stream.
    detune: Param,
    /// Indicates if the region of audio data designated by loopStart and loopEnd
    /// should be played continuously in a loop.
    loop_enabled: bool,
    /// An playhead position where looping should end if the loop_enabled
    /// attribute is true.
    loop_end: Option<f64>,
    /// An playhead position where looping should begin if the loop_enabled
    /// attribute is true.
    loop_start: Option<f64>,
    /// The speed at which to render the audio stream. Can be negative if the
    /// audio is to be played backwards. With a negative playback_rate, looping
    /// jumps from loop_start to loop_end instead of the other way around.
    playback_rate: Param,
    /// Time at which the source should start playing.
    start_at: Option<Tick>,
    /// Time at which the source should stop playing.
    stop_at: Option<Tick>,
    /// The ended event callback.
    pub onended_callback: Option<OnEndedCallback>,
}

impl AudioBufferSourceNode {
    pub fn new(options: AudioBufferSourceNodeOptions, channel_info: ChannelInfo) -> Self {
        Self {
            channel_info,
            buffer: options.buffer,
            buffer_pos: 0.,
            detune: Param::new(options.detune),
            loop_enabled: options.loop_enabled,
            loop_end: options.loop_end,
            loop_start: options.loop_start,
            playback_rate: Param::new(options.playback_rate),
            start_at: None,
            stop_at: None,
            onended_callback: None,
        }
    }

    pub fn handle_message(&mut self, message: AudioBufferSourceNodeMessage, _: f32) {
        match message {
            AudioBufferSourceNodeMessage::SetBuffer(buffer) => {
                self.buffer = buffer;
            }
        }
    }
}

impl AudioNodeEngine for AudioBufferSourceNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::AudioBufferSourceNode
    }

    fn input_count(&self) -> u32 {
        0
    }

    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 0);

        if self.buffer.is_none() {
            inputs.blocks.push(Default::default());
            return inputs;
        }

        let (start_at, stop_at) = match self.should_play_at(info.frame) {
            ShouldPlay::No => {
                inputs.blocks.push(Default::default());
                return inputs;
            }
            ShouldPlay::Between(start, end) => (start.0 as usize, end.0 as usize),
        };

        let buffer = self.buffer.as_ref().unwrap();

        // We will output at most this many frames (fewer if we run out of data).
        let frames_to_output = stop_at - start_at;

        self.playback_rate.update(info, Tick(0));
        self.detune.update(info, Tick(0));
        // computed_playback_rate can be negative or zero.
        let computed_playback_rate =
            self.playback_rate.value() as f64 * (2.0_f64).powf(self.detune.value() as f64 / 1200.);
        let buffer_offset_per_tick =
            computed_playback_rate * (buffer.sample_rate as f64 / info.sample_rate as f64);
        let forward = computed_playback_rate >= 0.;

        let (mut actual_loop_start, mut actual_loop_end) = (0., buffer.len() as f64);
        if self.loop_enabled {
            let loop_start = self.loop_start.unwrap_or(0.);
            let loop_end = self.loop_end.unwrap_or(0.);

            if loop_start >= 0. && loop_end > loop_start {
                actual_loop_start = loop_start * (buffer.sample_rate as f64);
                actual_loop_end = loop_end * (buffer.sample_rate as f64);
            }

            if forward && self.buffer_pos >= actual_loop_end {
                self.buffer_pos = actual_loop_start;
            }
            // XXX(collares): This is technically not in the spec, but it's the
            // only thing that makes sense. I suspect the spec was not fully
            // updated for negative playbackRates.
            if !forward && self.buffer_pos < actual_loop_start {
                self.buffer_pos = actual_loop_end;
            }
        }

        if self.loop_enabled && buffer_offset_per_tick.abs() < actual_loop_end - actual_loop_start {
            // Refuse to output data in this extreme edge case.
            //
            // XXX(collares): There are two ways we could handle it:
            // 1) Take buffer_offset_per_tick modulo the loop length, and handle
            // the pre-loop-entering output separately.
            // 2) Add a division by the loop length to the hot path below.
            // None of them seem worth the trouble. The spec should forbid this.
            self.maybe_trigger_onended_callback();
            inputs.blocks.push(Default::default());
            return inputs;
        }

        // Fast path for the case where we can just copy FRAMES_PER_BLOCK
        // frames straight from the buffer.
        if frames_to_output == FRAMES_PER_BLOCK.0 as usize
            && forward
            && buffer_offset_per_tick == 1.
            && self.buffer_pos.trunc() == self.buffer_pos
            && self.buffer_pos + (FRAMES_PER_BLOCK.0 as f64) <= actual_loop_end
        {
            let mut block = Block::empty();
            let pos = self.buffer_pos as usize;

            for chan in 0..buffer.chans() {
                block.push_chan(&buffer.buffers[chan as usize][pos..(pos + frames_to_output)]);
            }

            inputs.blocks.push(block);
            self.buffer_pos += FRAMES_PER_BLOCK.0 as f64;
        } else {
            // Slow path, with interpolation.
            let mut block = Block::default();
            block.repeat(buffer.chans());
            block.explicit_repeat();

            debug_assert!(buffer.chans() > 0);

            for chan in 0..buffer.chans() {
                let data = block.data_chan_mut(chan);
                let (_, data) = data.split_at_mut(start_at);
                let (data, _) = data.split_at_mut(frames_to_output);

                let mut pos = self.buffer_pos;

                for sample in data {
                    if self.loop_enabled {
                        if forward && pos >= actual_loop_end {
                            pos -= actual_loop_end - actual_loop_start;
                        } else if !forward && pos < actual_loop_start {
                            pos += actual_loop_end - actual_loop_start;
                        }
                    } else if pos < 0. || pos >= buffer.len() as f64 {
                        break;
                    }

                    *sample = buffer.interpolate(chan, pos);
                    pos += buffer_offset_per_tick;
                }

                if chan == buffer.chans() - 1 {
                    self.buffer_pos = pos;
                }
            }

            inputs.blocks.push(block);
        }

        if self.buffer_pos < 0. || self.buffer_pos >= buffer.len() as f64 {
            self.maybe_trigger_onended_callback();
        }

        inputs
    }

    fn get_param(&mut self, id: ParamType) -> &mut Param {
        match id {
            ParamType::PlaybackRate => &mut self.playback_rate,
            ParamType::Detune => &mut self.detune,
            _ => panic!("Unknown param {:?} for AudioBufferSourceNode", id),
        }
    }

    make_message_handler!(
        AudioBufferSourceNode: handle_message,
        AudioScheduledSourceNode: handle_source_node_message
    );
}

#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// Invariant: all buffers must be of the same length
    pub buffers: Vec<Vec<f32>>,
    pub sample_rate: f32,
}

impl AudioBuffer {
    pub fn new(chan: u8, len: usize, sample_rate: f32) -> Self {
        assert!(chan > 0);
        let mut buffers = Vec::with_capacity(chan as usize);
        let single = vec![0.; len];
        buffers.resize(chan as usize, single);
        AudioBuffer {
            buffers,
            sample_rate,
        }
    }

    pub fn from_buffers(buffers: Vec<Vec<f32>>, sample_rate: f32) -> Self {
        for buf in &buffers {
            assert_eq!(buf.len(), buffers[0].len())
        }

        Self {
            buffers,
            sample_rate,
        }
    }

    pub fn from_buffer(buffer: Vec<f32>, sample_rate: f32) -> Self {
        AudioBuffer::from_buffers(vec![buffer], sample_rate)
    }

    pub fn len(&self) -> usize {
        self.buffers[0].len()
    }

    pub fn chans(&self) -> u8 {
        self.buffers.len() as u8
    }

    // XXX(collares): There are better fast interpolation algorithms.
    // Firefox uses (via Speex's resampler) the algorithm described in
    // https://ccrma.stanford.edu/~jos/resample/resample.pdf
    pub fn interpolate(&self, chan: u8, pos: f64) -> f32 {
        debug_assert!(pos >= 0. && pos < self.len() as f64);

        let prev = pos.floor() as usize;
        let offset = pos - pos.floor();
        let next_sample = *self.buffers[chan as usize].get(prev + 1).unwrap_or(&0.0);

        ((1. - offset) * (self.buffers[chan as usize][prev] as f64) + offset * (next_sample as f64))
            as f32
    }

    pub fn data_chan_mut(&mut self, chan: u8) -> &mut [f32] {
        &mut self.buffers[chan as usize]
    }
}
