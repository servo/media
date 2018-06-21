use audio::graph_impl::PortIndex;
use byte_slice_cast::*;
use smallvec::SmallVec;
use std::ops::*;
use std::mem;

// defined by spec
// https://webaudio.github.io/web-audio-api/#render-quantum
pub const FRAMES_PER_BLOCK: Tick = Tick(128);
const FRAMES_PER_BLOCK_USIZE: usize = FRAMES_PER_BLOCK.0 as usize;

/// A tick, i.e. the time taken for a single frame
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub struct Tick(pub u64);

/// A collection of blocks received as input by a node
/// or outputted by a node.
///
/// This will usually be a single block.
///
/// Some nodes have multiple inputs or outputs, which is
/// where this becomes useful. Source nodes have an input
/// of an empty chunk.
pub struct Chunk {
    pub blocks: SmallVec<[Block; 1]>,
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk {
            blocks: SmallVec::new(),
        }
    }
}

impl Chunk {
    pub fn len(&self) -> usize {
        self.blocks.len()
    }
}

/// We render audio in blocks of size FRAMES_PER_BLOCK
///
/// A single block may contain multiple channels
#[derive(Clone)]
pub struct Block {
    /// The number of channels in this block
    channels: u8,
    /// This is an optimization which means that the buffer is representing multiple channels with the 
    /// same content at once. Happens when audio is upmixed or when a source like
    /// an oscillator node has multiple channel outputs
    repeat: bool,
    /// If this vector is empty, it is a shorthand for "silence"
    /// It is possible to obtain an explicitly silent buffer via .explicit_silence()
    ///
    /// This must be of length channels * FRAMES_PER_BLOCK, unless `repeat` is true,
    /// in which case it will be of length FRAMES_PER_BLOCK
    buffer: Vec<f32>,
}


impl Default for Block {
    fn default() -> Self {
        Block {
            channels: 1,
            repeat: false,
            buffer: Vec::new(),
        }
    }
}

impl Block {
    /// This provides the entire buffer as a mutable slice of u8
    pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        self.data_mut().as_mut_byte_slice().expect("casting failed")
    }

    /// If this is in "silence" mode without a buffer, allocate a silent buffer
    pub fn explicit_silence(&mut self) {
        if self.buffer.is_empty() {
            self.buffer.resize(FRAMES_PER_BLOCK_USIZE, 0.);
            self.repeat = true;
        }
    }

    /// This provides the entire buffer as a mutable slice of f32
    pub fn data_mut(&mut self) -> &mut [f32] {
        self.explicit_silence();
        &mut self.buffer
    }

    pub fn explicit_repeat(&mut self) {
        if self.repeat && self.channels > 1 {
            let mut new = Vec::with_capacity(FRAMES_PER_BLOCK_USIZE * self.channels as usize);
            for _ in 0..self.channels {
                new.extend(&self.buffer)
            }

            self.buffer = new;
        } else {
            self.explicit_silence()
        }
    }

    pub fn data_chan_mut(&mut self, chan: u8) -> &mut [f32] {
        self.explicit_repeat();
        let start = chan as usize * FRAMES_PER_BLOCK_USIZE;
        &mut self.buffer[start..start + FRAMES_PER_BLOCK_USIZE]
    }

    pub fn take(&mut self) -> Block {
        let mut new = Block::default();
        new.channels = self.channels;
        mem::replace(self, new)
    }

    pub fn chan_count(&self) -> u8 {
        self.channels
    }
}

impl<T> IndexMut<PortIndex<T>> for Chunk {
    fn index_mut(&mut self, i: PortIndex<T>) -> &mut Block {
        &mut self.blocks[i.0 as usize]
    }
}

impl<T> Index<PortIndex<T>> for Chunk {
    type Output = Block;
    fn index(&self, i: PortIndex<T>) -> &Block {
        &self.blocks[i.0 as usize]
    }
}

impl Add<Tick> for Tick {
    type Output = Tick;
    fn add(self, other: Tick) -> Self {
        self + other.0
    }
}

impl AddAssign for Tick {
    fn add_assign(&mut self, other: Tick) {
        *self = *self + other
    }
}

impl Sub<Tick> for Tick {
    type Output = Tick;
    fn sub(self, other: Tick) -> Self {
        self - other.0
    }
}

impl Add<u64> for Tick {
    type Output = Tick;
    fn add(self, other: u64) -> Self {
        Tick(self.0 + other)
    }
}

impl Sub<u64> for Tick {
    type Output = Tick;
    fn sub(self, other: u64) -> Self {
        Tick(self.0 - other)
    }
}

impl Div<f64> for Tick {
    type Output = f64;
    fn div(self, other: f64) -> f64 {
        self.0 as f64 / other
    }
}

impl Tick {
    pub fn from_time(time: f64, rate: f32) -> Tick {
        Tick((time * rate as f64) as u64)
    }

    pub fn advance(&mut self) {
        self.0 += 1;
    }
}
