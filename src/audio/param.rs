use audio::block::Tick;
use audio::node::BlockInfo;

/// An AudioParam. 
///
/// https://webaudio.github.io/web-audio-api/#AudioParam
#[derive(Copy, Clone)]
pub struct Param {
    val: f64,
    kind: ParamKind
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ParamKind {
    /// Value is held for entire block
    KRate,
    /// Value is updated each frame
    ARate,
}


impl Param {
    pub fn new(val: f64, kind: ParamKind) -> Self{
        Param {
            val, kind
        }
    }

    /// Update the value of this param to the next
    pub fn update(&mut self, _block: &BlockInfo, tick: Tick) {
        if tick.0 != 0 && self.kind == ParamKind::KRate {
            return;
        }
        // fun stuff goes here
    }

    pub fn value(&self) -> f64 {
        self.val
    }
}
