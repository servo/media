use std::ops::*;
use smallvec::SmallVec;
use byte_slice_cast::*;

// defined by spec
// https://webaudio.github.io/web-audio-api/#render-quantum
pub const FRAMES_PER_BLOCK: Tick = Tick(128);

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
    // None means silence, will be lazily filled
    data: Option<Box<[f32]>>,
}

impl Default for Block {
    fn default() -> Self {
        Block {
            data: None
        }
    }
}

impl Block {
    pub fn as_mut_byte_slice(&mut self) -> &mut [u8] {
        self.data_mut().as_mut_byte_slice().expect("casting failed")
    }

    pub fn explicit_silence(&mut self) {
        if self.data.is_none() {
            self.data = Some(Box::new([0.; FRAMES_PER_BLOCK.0 as usize]))
        }
    }

    pub fn data_mut(&mut self) -> &mut [f32] {
        self.explicit_silence();
        &mut ** self.data.as_mut().unwrap()
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