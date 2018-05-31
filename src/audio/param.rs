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
    event_start_time: Tick,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ParamKind {
    /// Value is held for entire block
    KRate,
    /// Value is updated each frame
    ARate,
}


impl Param {
    pub fn new(val: f64) -> Self{
        Param {
            val,
            kind: ParamKind::ARate,
            events: vec![],
            current_event: 0,
            event_start_time: Tick(0),
        }
    }

    /// Update the value of this param to the next
    pub fn update(&mut self, block: &BlockInfo, tick: Tick) {
        if tick.0 != 0 && self.kind == ParamKind::KRate {
            return;
        }

        // println!("Curr {:?}", self.current_event);
        if self.events.len() <= self.current_event  {
            return;
        }


        let current_tick = block.absolute_tick(tick);
        // println!("curr_tick {:?}", current_tick);
        let mut current_event = &self.events[self.current_event];

        // move to next event if necessary
        // XXXManishearth k-rate events may get skipped over completely by this
        // method. Firefox currently doesn't support these, however, so we can
        // handle those later
        if let Some(done_time) = current_event.done_time() {
            if done_time < current_tick {
                self.current_event += 1;
                self.event_start_time = current_tick;
                if let Some(next) = self.events.get(self.current_event) {
                    current_event = next;
                } else {
                    return;
                }
            }
        } else if let Some(next) = self.events.get(self.current_event + 1) {
            if let Some(start_time) = next.start_time() {
                if start_time >= current_tick {
                    self.current_event += 1;
                    self.event_start_time = current_tick;
                    current_event = next;
                }
            }
        }

        current_event.run(&mut self.val, current_tick, self.event_start_time);
    }

    pub fn value(&self) -> f64 {
        self.val
    }

    pub fn insert_event(&mut self, event: AutomationEvent) {
        let time = event.time();

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
        }
    }

    pub fn done_time(&self) -> Option<Tick> {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => Some(tick),
        }
    }

    pub fn start_time(&self) -> Option<Tick> {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => Some(tick),
        }
    }

    pub fn run(&self, value: &mut f64, current_tick: Tick, _event_start_time: Tick) {
        match *self {
            AutomationEvent::SetValueAtTime(val, time) => {
                if current_tick == time {
                    *value = val
                }
            }
        }
    }
}
