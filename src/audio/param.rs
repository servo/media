use audio::block::Tick;
use audio::node::BlockInfo;

/// An AudioParam. 
///
/// https://webaudio.github.io/web-audio-api/#AudioParam
#[derive(Debug)]
pub struct Param {
    val: f32,
    kind: ParamKind,
    events: Vec<AutomationEvent>,
    current_event: usize,
    event_start_time: Tick,
    event_start_value: f32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ParamKind {
    /// Value is held for entire block
    KRate,
    /// Value is updated each frame
    ARate,
}


impl Param {
    pub fn new(val: f32) -> Self{
        Param {
            val,
            kind: ParamKind::ARate,
            events: vec![],
            current_event: 0,
            event_start_time: Tick(0),
            event_start_value: val,
        }
    }

    /// Update the value of this param to the next
    ///
    /// Returns true if anything changed
    pub fn update(&mut self, block: &BlockInfo, tick: Tick) -> bool {
        if tick.0 != 0 && self.kind == ParamKind::KRate {
            return false;
        }

        // println!("Curr {:?}", self.current_event);
        if self.events.len() <= self.current_event  {
            return false;
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
                self.event_start_value = self.val;
                self.event_start_time = current_tick;
                if let Some(next) = self.events.get(self.current_event) {
                    current_event = next;
                } else {
                    return false;
                }
            }
        } else if let Some(next) = self.events.get(self.current_event + 1) {
            if let Some(start_time) = next.start_time() {
                if start_time >= current_tick {
                    self.current_event += 1;
                    self.event_start_value = self.val;
                    self.event_start_time = current_tick;
                    current_event = next;
                }
            }
        }

        current_event.run(&mut self.val, current_tick,
                          self.event_start_time,
                          self.event_start_value)
    }

    pub fn value(&self) -> f32 {
        self.val
    }

    pub(crate) fn insert_event(&mut self, event: AutomationEvent) {
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
pub(crate) enum AutomationEvent {

    SetValueAtTime(f32, Tick),
    RampToValueAtTime(RampKind, f32, Tick),
    // SetTargetAtTime(f32, Tick, /* time constant, units of 1/Tick */ f64),
    // SetValueCurveAtTime(Vec<f32>, Tick, /* duration */ Tick)
    // CancelAndHoldAtTime(Tick),
}


#[derive(Clone, Copy, PartialEq, Debug)]
/// An AutomationEvent that uses times in s instead of Ticks
pub enum UserAutomationEvent {

    SetValueAtTime(f32, /* time */ f64),
    RampToValueAtTime(RampKind, f32, /* time */ f64),
    // SetTargetAtTime(f32, Tick, /* time constant, units of 1/Tick */ f64),
    // SetValueCurveAtTime(Vec<f32>, Tick, /* duration */ Tick)
    // CancelAndHoldAtTime(Tick),
}

impl UserAutomationEvent {
    pub(crate) fn to_event(self, rate: f32) -> AutomationEvent {
        match self {
            UserAutomationEvent::SetValueAtTime(val, time) =>
                AutomationEvent::SetValueAtTime(val, Tick::from_time(time, rate)),
            UserAutomationEvent::RampToValueAtTime(kind, val, time) =>
                AutomationEvent::RampToValueAtTime(kind, val, Tick::from_time(time, rate))
        }
    }
}

impl AutomationEvent {
    /// The time of the event used for ordering
    pub fn time(&self) -> Tick {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => tick,
            AutomationEvent::RampToValueAtTime(_, _, tick) => tick,
        }
    }

    pub fn done_time(&self) -> Option<Tick> {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => Some(tick),
            AutomationEvent::RampToValueAtTime(_, _, tick) => Some(tick),
        }
    }

    pub fn start_time(&self) -> Option<Tick> {
        match *self {
            AutomationEvent::SetValueAtTime(_, tick) => Some(tick),
            AutomationEvent::RampToValueAtTime(..) => None,
        }
    }

    /// Update a parameter based on this event
    ///
    /// Returns true if something changed
    pub fn run(&self, value: &mut f32,
               current_tick: Tick,
               event_start_time: Tick,
               event_start_value: f32) -> bool {
        match *self {
            AutomationEvent::SetValueAtTime(val, time) => {
                if current_tick == time {
                    *value = val;
                    true
                } else {
                    false
                }
            }
            AutomationEvent::RampToValueAtTime(kind, val, time) => {
                let progress = (current_tick - event_start_time).0 as f32 /
                               (time - event_start_time).0 as f32;
                match kind {
                    RampKind::Linear => {
                        *value = event_start_value + (val - event_start_value) * progress;
                    }
                    RampKind::Exponential => {
                        *value = event_start_value * (val / event_start_value).powf(progress);
                    }
                }
                true
            }
        }
    }
}
