use block::{Block, Chunk, FRAMES_PER_BLOCK_USIZE};
use node::AudioNodeEngine;
use node::BlockInfo;
use node::{AudioNodeType, ChannelInfo, ChannelInterpretation};
use std::f32::consts::PI;
use std::sync::mpsc::Sender;
use std::mem;

#[derive(AudioNodeCommon)]
pub(crate) struct AnalyserNode {
    channel_info: ChannelInfo,
    sender: Sender<Block>
}

impl AnalyserNode {
    pub fn new(sender: Sender<Block>, channel_info: ChannelInfo) -> Self {
        Self { sender, channel_info }
    }

}

impl AudioNodeEngine for AnalyserNode {
    fn node_type(&self) -> AudioNodeType {
        AudioNodeType::AnalyserNode
    }

    fn process(&mut self, inputs: Chunk, _: &BlockInfo) -> Chunk {
        debug_assert!(inputs.len() == 1);

        let mut push = inputs.blocks[0].clone();
        push.mix(1, ChannelInterpretation::Speakers);

        let _ = self.sender.send(push);

        // analyser node doesn't modify the inputs
        inputs
    }
}

/// From https://webaudio.github.io/web-audio-api/#dom-analysernode-fftsize
pub const MAX_FFT_SIZE: usize = 32768;
pub const MAX_BLOCK_COUNT: usize = MAX_FFT_SIZE / FRAMES_PER_BLOCK_USIZE;

/// The actual analysis is done on the DOM side. We provide
/// the actual base functionality in this struct, so the DOM
/// just has to do basic shimming
pub struct AnalysisEngine {
    /// The number of past sample-frames to consider in the FFT
    fft_size: usize,
    /// This is a ring buffer containing the last MAX_FFT_SIZE
    /// sample-frames 
    data: Box<[f32; MAX_FFT_SIZE]>,
    /// The index of the current block
    current_block: usize,
    /// Have we computed the FFT already?
    fft_computed: bool,
    /// Cached blackman window data
    blackman_windows: Vec<f32>,
    /// The computed FFT data (in frequency domain)
    computed_fft_data: Vec<f32>,

    // these two vectors are for temporary buffers
    // that we keep around for efficiency

    /// The windowed time domain data
    /// Used during FFT computation
    windowed: Vec<f32>,
    /// Scratch space used during the actual FFT computation
    tmp_transformed: Vec<f32> 
}

impl AnalysisEngine {
    pub fn new(fft_size: usize) -> Self {
        Self {
            fft_size,
            data: Box::new([0.; MAX_FFT_SIZE]),
            current_block: MAX_BLOCK_COUNT - 1,
            fft_computed: false,
            blackman_windows: Vec::with_capacity(fft_size),
            computed_fft_data: Vec::with_capacity(fft_size / 2),
            windowed: Vec::with_capacity(fft_size),
            tmp_transformed: Vec::with_capacity(fft_size / 2),
        }
    }

    fn advance(&mut self) {
        self.current_block += 1;
        if self.current_block >= MAX_BLOCK_COUNT {
            self.current_block = 0;
        }
    }

    /// Wrap around the index of a block `offset` elements in the past
    fn block_index(&self, offset: usize) -> usize {
        debug_assert!(offset < MAX_BLOCK_COUNT);
        if offset > self.current_block {
            MAX_BLOCK_COUNT - offset + self.current_block
        } else {
            self.current_block - offset
        }   
    }

    /// Get the data of a block. `offset` tells us how far back to go
    fn block_mut(&mut self, offset: usize) -> &mut [f32] {
        let index = FRAMES_PER_BLOCK_USIZE * self.block_index(offset);
        &mut self.data[index..(index + FRAMES_PER_BLOCK_USIZE)]
    }

    pub fn push(&mut self, mut block: Block) {
        debug_assert!(block.chan_count() == 1);
        self.advance();
        if !block.is_silence() {
            self.block_mut(0).copy_from_slice(block.data_mut());
        }
        self.fft_computed = false;
    }

    /// https://webaudio.github.io/web-audio-api/#blackman-window
    fn compute_blackman_windows(&mut self) {
        if self.blackman_windows.len() == self.fft_size {
            return;
        }
        const ALPHA: f32 = 0.16;
        const ALPHA_0: f32 = (1. - ALPHA) / 2.;
        const ALPHA_1: f32 = 1. / 2.;
        const ALPHA_2: f32 = ALPHA / 2.;
        self.blackman_windows.resize(self.fft_size, 0.);
        let coeff = PI * 2. / self.fft_size as f32;
        for n in 0..self.fft_size {
            self.blackman_windows[n] = ALPHA_0 - ALPHA_1 * (coeff * n as f32).cos() 
                                               + ALPHA_2 * (2. * coeff * n as f32).cos();
        }
    }

    fn apply_blackman_window(&mut self) {
        self.compute_blackman_windows();
        // avoids conflicting borrows with data_mut
        let mut windowed = mem::replace(&mut self.windowed, Vec::new());
        windowed.resize(self.fft_size, 0.);
        let mut n = 0;
        for offset in (0..self.fft_size).rev() {
            let data = self.block_mut(offset);
            for frame in 0..FRAMES_PER_BLOCK_USIZE {
                windowed[n] = data[frame];
                n += 1;
            }
        }
        self.windowed = windowed;
    }

    fn compute_fft(&mut self) {
        if self.fft_computed {
            return;
        }
        self.fft_computed = true;
        self.apply_blackman_window();
        // ...
    }
}
