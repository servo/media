use std::ops::*;
use smallvec::SmallVec;
use byte_slice_cast::*;

// defined by spec
// https://webaudio.github.io/web-audio-api/#render-quantum
pub const FRAMES_PER_BLOCK: Tick = Tick(128);

/// A tick, i.e. the time taken for a single frame
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Tick(pub u32);

/// A collection of blocks received as input by a node
/// or outputted by a node.
///
/// This will usually be a single block.
///
/// Some nodes have multiple inputs or outputs, which is
/// where this becomes useful. Source nodes have an input
/// of an empty chunk.
pub struct Chunk {
    pub blocks: SmallVec<[Block; 1]>
}

impl Default for Chunk {
    fn default() -> Self {
        Chunk {
            blocks: SmallVec::new()
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
pub struct Block {
    // todo: handle channels
    pub data: Box<[f32]>,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            data: Box::new([0.; FRAMES_PER_BLOCK.0 as usize])
        }
    }
}

impl Block {
    pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        self.data.as_mut_byte_slice().expect("casting failed")
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

impl Add<u32> for Tick {
    type Output = Tick;
    fn add(self, other: u32) -> Self {
        Tick(self.0 + other)
    }
}

impl Sub<u32> for Tick {
    type Output = Tick;
    fn sub(self, other: u32) -> Self {
        Tick(self.0 - other)
    }
}

impl Div<f64> for Tick {
    type Output = f64;
    fn div(self, other: f64) -> f64 {
        self.0 as f64 / other
    }
}