use audio::block::Tick;
use audio::node::BlockInfo;

/// An AudioParam. 
///
/// https://webaudio.github.io/web-audio-api/#AudioParam
#[derive(Debug)]
pub struct Param {
    val: f64,
    kind: ParamKind,
    events: Vec<AutomationEvent>,
    current_event: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ParamKind {
    /// Value is held for entire block
    KRate,
    /// Value is updated each frame
    ARate,
}


impl Param {
    pub fn new(val: f64, kind: ParamKind) -> Self{
        Param {
            val, kind,
            events: vec![],
            current_event: 0,
        }
    }

    /// Update the value of this param to the next
    pub fn update(&mut self, block: &BlockInfo, tick: Tick) {
        if tick.0 != 0 && self.kind == ParamKind::KRate {
            return;
        }

        if self.events.len() == 0 {
            return;
        }
        let tick = block.absolute_tick(tick);
    }

    pub fn value(&self) -> f64 {
        self.val
    }

    pub fn insert_event(&mut self, event: AutomationEvent) {
        let time = event.time();
        if self.events.len() == 0 {
            self.events.push(AutomationEvent::Hold(Tick(0)));
            self.events.push(event);
            return;
        }
        let result = self.events.binary_search_by(|e| e.time().cmp(&time));
        // XXXManishearth this should handle overlapping events
        let idx = match result {
            Ok(idx) => idx,
            Err(idx) => idx,
        };
        self.events.insert(idx, event);
        // XXXManishearth handle inserting events with a time before that
        // of the current one
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum RampKind {
    Linear, Exponential
}

#[derive(Clone, Copy, PartialEq, Debug)]
/// https://webaudio.github.io/web-audio-api/#dfn-automation-event
pub enum AutomationEvent {
    /// Stay constant
    Hold(Tick),
    SetValueAtTime(f64, Tick),
    // RampToValueAtTime(RampKind, f64, Tick),
    // SetTargetAtTime(f64, Tick, /* time constant, units of 1/Tick */ f64),
    // SetValueCurveAtTime(Vec<f64>, Tick, /* duration */ Tick)
    // CancelAndHoldAtTime(Tick),
}

impl AutomationEvent {
    /// The time of the event used for ordering
    pub fn time(&self) -> Tick {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => tick,
            AutomationEvent::Hold(tick) => tick,
        }
    }
}
