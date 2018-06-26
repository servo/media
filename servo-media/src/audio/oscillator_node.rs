use audio::node::ChannelCountMode;
use audio::block::{Chunk, Tick};
use audio::node::{AudioNodeEngine, AudioScheduledSourceNodeMessage, BlockInfo};
use audio::param::{Param, UserAutomationEvent};
use num_traits::cast::NumCast;

pub enum OscillatorNodeMessage {
    SetFrequency(UserAutomationEvent),
}

#[derive(Copy, Clone)]
pub struct PeriodicWaveOptions {
    // XXX https://webaudio.github.io/web-audio-api/#dictdef-periodicwaveoptions
}

#[derive(Copy, Clone)]
pub enum OscillatorType {
    Sine,
    Square,
    Sawtooth,
    Triangle,
    Custom,
}

#[derive(Copy, Clone)]
pub struct OscillatorNodeOptions {
    pub oscillator_type: OscillatorType,
    pub freq: f32,
    pub detune: f32,
    pub periodic_wave_options: Option<PeriodicWaveOptions>,
}

impl Default for OscillatorNodeOptions {
    fn default() -> Self {
        OscillatorNodeOptions {
            oscillator_type: OscillatorType::Sine,
            freq: 440.,
            detune: 0.,
            periodic_wave_options: None,
        }
    }
}

#[derive(AudioScheduledSourceNode)]
pub struct OscillatorNode {
    frequency: Param,
    phase: f64,
    /// Time at which the source should start playing.
    start_at: Option<Tick>,
    /// Time at which the source should stop playing.
    stop_at: Option<Tick>,
}

impl OscillatorNode {
    pub fn new(options: OscillatorNodeOptions) -> Self {
        Self {
            frequency: Param::new(options.freq.into()),
            phase: 0.,
            start_at: None,
            stop_at: None,
        }
    }

    pub fn update_parameters(&mut self, info: &BlockInfo, tick: Tick) -> bool {
        self.frequency.update(info, tick)
    }

    pub fn handle_message(&mut self, message: OscillatorNodeMessage, sample_rate: f32) {
        match message {
            OscillatorNodeMessage::SetFrequency(event) => {
                self.frequency.insert_event(event.to_event(sample_rate))
            }
        }
    }

    pub fn handle_source_node_message(&mut self, message: AudioScheduledSourceNodeMessage, sample_rate: f32) {
        match message {
            AudioScheduledSourceNodeMessage::Start(when) => {
                self.start(Tick::from_time(when, sample_rate));
            }
            AudioScheduledSourceNodeMessage::Stop(when) => {
                self.stop(Tick::from_time(when, sample_rate));
            }
        }
    }
}

impl AudioNodeEngine for OscillatorNode {
    fn process(&mut self, mut inputs: Chunk, info: &BlockInfo) -> Chunk {
        // XXX Implement this properly and according to self.options
        // as defined in https://webaudio.github.io/web-audio-api/#oscillatornode

        use std::f64::consts::PI;

        debug_assert!(inputs.len() == 0);

        inputs.blocks.push(Default::default());

        if self.should_play_at(info.frame) == (false, true) {
            return inputs;
        }

        {

            inputs.blocks[0].explicit_silence();
            let mut iter = inputs.blocks[0].iter();

            // Convert all our parameters to the target type for calculations
            let vol: f32 = 1.0;
            let freq = self.frequency.value() as f64;
            let sample_rate = info.sample_rate as f64;
            let two_pi = 2.0 * PI;

            // We're carrying a phase with up to 2pi around instead of working
            // on the sample offset. High sample offsets cause too much inaccuracy when
            // converted to floating point numbers and then iterated over in 1-steps
            //
            // Also, if the frequency changes the phase should not
            let mut step = two_pi * freq / sample_rate;
            while let Some(mut frame) = iter.next() {
                let tick = frame.tick();
                let (should_play_at, should_break) = self.should_play_at(info.frame + tick);
                if !should_play_at {
                    if should_break {
                        break;
                    }
                    continue;
                }
                if self.update_parameters(info, tick) {
                    step = two_pi * freq / sample_rate;
                }
                let value = vol * f32::sin(NumCast::from(self.phase).unwrap());
                frame.mutate_with(|sample| *sample = value);

                self.phase += step;
                if self.phase >= two_pi {
                    self.phase -= two_pi;
                }
            }
        }
        inputs
    }

    fn input_count(&self) -> u32 {
        0
    }

    fn channel_count_mode(&self) -> ChannelCountMode {
        ChannelCountMode::Max
    }

    make_message_handler!(AudioScheduledSourceNode: handle_source_node_message,
                          OscillatorNode: handle_message);
}
