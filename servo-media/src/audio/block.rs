use audio::node::ChannelInterpretation;
use audio::graph::PortIndex;
use byte_slice_cast::*;
use smallvec::SmallVec;
use std::ops::*;
use std::mem;

// defined by spec
// https://webaudio.github.io/web-audio-api/#render-quantum
pub const FRAMES_PER_BLOCK: Tick = Tick(128);
pub const FRAMES_PER_BLOCK_USIZE: usize = FRAMES_PER_BLOCK.0 as usize;

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
        } else if self.is_silence() {
            self.buffer.resize(FRAMES_PER_BLOCK_USIZE * self.channels as usize, 0.);
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

    pub fn iter(&mut self) -> FrameIterator {
        FrameIterator::new(self)
    }

    pub fn is_silence(&self) -> bool {
        self.buffer.is_empty()
    }

    /// upmix/downmix the channels if necessary
    ///
    /// Currently only supports upmixing from 1
    pub fn mix(&mut self, channels: u8, interpretation: ChannelInterpretation) {
        if self.channels == channels {
            return
        }

        assert!(self.channels == 1 && channels > 1);
        self.channels = channels;
        if !self.is_silence() {
            self.repeat = true;
        }
    }

    /// Take a single-channel block and repeat the
    /// channel
    pub fn repeat(&mut self, channels: u8) {
        debug_assert!(self.channels == 1);
        self.channels = channels;
        if !self.is_silence() {
            self.repeat = true;
        }
    }

    pub fn interleave(&mut self) -> Vec<f32> {
        self.explicit_repeat();
        let mut vec = Vec::with_capacity(self.buffer.len());
        // FIXME this isn't too efficient
        vec.resize(self.buffer.len(), 0.);
        for frame in 0..FRAMES_PER_BLOCK_USIZE {
            let channels = self.channels as usize;
            for chan in 0..channels {
                vec[frame * channels + chan] = self.buffer[chan * FRAMES_PER_BLOCK_USIZE + frame]
            }
        }
        vec
    }
}

/// An iterator over frames in a block
pub struct FrameIterator<'a> {
    frame: Tick,
    block: &'a mut Block
}

impl<'a> FrameIterator<'a> {
    #[inline]
    pub fn new(block: &'a mut Block) -> Self {
        FrameIterator {
            frame: Tick(0),
            block
        }
    }

    /// Advance the iterator
    ///
    /// We can't implement Iterator since it doesn't support
    /// streaming iterators, but we can call `while let Some(frame) = iter.next()`
    /// here
    #[inline]
    pub fn next<'b>(&'b mut self) -> Option<FrameRef<'b>> {
        let curr = self.frame;
        if curr < FRAMES_PER_BLOCK {
            self.frame.advance();
            Some(FrameRef { frame: curr, block: &mut self.block })
        } else {
            None
        }
    }
}


/// A reference to a frame
pub struct FrameRef<'a> {
    frame: Tick,
    block: &'a mut Block
}

impl<'a> FrameRef<'a> {
    #[inline]
    pub fn tick(&self) -> Tick {
        self.frame
    }

    /// Given a block and a function `f`, mutate the frame through all channels with `f`
    ///
    /// Use this when you plan to do the same operation for each channel.
    /// (Helpers for the other cases will eventually exist)
    ///
    /// Block must not be silence
    #[inline]
    pub fn mutate_with<F>(&mut self, f: F) where F: Fn(&mut f32) {
        debug_assert!(!self.block.is_silence(), "mutate_frame_with should not be called with a silenced block, \
                                                 call .explicit_silence() if you wish to use this");
        if self.block.repeat {
            f(&mut self.block.buffer[self.frame.0 as usize])
        } else {
            for chan in 0..self.block.channels {
                f(&mut self.block.buffer[chan as usize * FRAMES_PER_BLOCK_USIZE + self.frame.0 as usize])
            }
        }
    }
}


// operator impls

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
