//! Deterministic adaptive-hotkey reducer.
//!
//! This module deliberately has no Tauri or Tokio dependency. The flow
//! coordinator serializes physical/direct input through it, then performs the
//! returned effect asynchronously. Keeping input interpretation pure makes the
//! 700 ms tap/hold boundary and key-repeat behaviour independently testable.

use std::collections::BTreeSet;

pub const HOLD_THRESHOLD_MS: u64 = 700;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Idle,
    Active,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Start,
    Stop,
    Latch,
    Busy,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    trigger_id: String,
    down_ms: u64,
}

#[derive(Debug, Default)]
pub struct AdaptiveReducer {
    held: BTreeSet<String>,
    origin: Option<Origin>,
}

impl AdaptiveReducer {
    /// Interpret a physical key-down. A key remains latched in `held` until its
    /// matching Up edge, so OS auto-repeat is ignored regardless of cadence.
    pub fn physical_down(
        &mut self,
        trigger_id: &str,
        at_ms: u64,
        availability: Availability,
    ) -> Decision {
        if !self.held.insert(trigger_id.to_owned()) {
            return Decision::Ignore;
        }

        match availability {
            Availability::Idle => {
                self.origin = Some(Origin {
                    trigger_id: trigger_id.to_owned(),
                    down_ms: at_ms,
                });
                Decision::Start
            }
            Availability::Active => {
                self.origin = None;
                Decision::Stop
            }
            Availability::Busy => Decision::Busy,
        }
    }

    /// Interpret a physical key-up using the edge timestamp captured at the
    /// callback boundary. This remains correct when audio startup is slow:
    /// startup completion can arrive later without changing the tap duration.
    pub fn physical_up(&mut self, trigger_id: &str, at_ms: u64) -> Decision {
        if !self.held.remove(trigger_id) {
            return Decision::Ignore;
        }

        let Some(origin) = self.origin.as_ref() else {
            return Decision::Ignore;
        };
        if origin.trigger_id != trigger_id {
            return Decision::Ignore;
        }

        if at_ms.saturating_sub(origin.down_ms) < HOLD_THRESHOLD_MS {
            Decision::Latch
        } else {
            self.origin = None;
            Decision::Stop
        }
    }

    /// Floater/Touch Bar controls are explicit toggles, not synthetic hotkeys.
    pub fn direct_toggle(&mut self, availability: Availability) -> Decision {
        match availability {
            Availability::Idle => {
                self.origin = None;
                Decision::Start
            }
            Availability::Active => {
                self.origin = None;
                Decision::Stop
            }
            Availability::Busy => Decision::Busy,
        }
    }

    pub fn escape(&mut self, availability: Availability) -> Decision {
        match availability {
            Availability::Active => {
                self.origin = None;
                Decision::Stop
            }
            Availability::Idle => Decision::Ignore,
            Availability::Busy => Decision::Busy,
        }
    }

    /// A terminal async result ends the logical input session. Physical keys
    /// deliberately remain in `held` until their Up edges arrive.
    pub fn session_ended(&mut self) {
        self.origin = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_is_strictly_less_than_700_for_tap() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(
            reducer.physical_down("F8", 1_000, Availability::Idle),
            Decision::Start
        );
        assert_eq!(reducer.physical_up("F8", 1_699), Decision::Latch);

        reducer.session_ended();
        assert_eq!(
            reducer.physical_down("F8", 2_000, Availability::Idle),
            Decision::Start
        );
        assert_eq!(reducer.physical_up("F8", 2_700), Decision::Stop);
    }

    #[test]
    fn repeat_is_latched_until_physical_up_even_after_debounce_windows() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(
            reducer.physical_down("F8", 0, Availability::Idle),
            Decision::Start
        );
        assert_eq!(
            reducer.physical_down("F8", 900, Availability::Active),
            Decision::Ignore
        );
        assert_eq!(reducer.physical_up("F8", 901), Decision::Stop);
    }

    #[test]
    fn queued_up_decides_while_starting() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(
            reducer.physical_down("F8", 10, Availability::Idle),
            Decision::Start
        );
        assert_eq!(reducer.physical_up("F8", 500), Decision::Latch);
    }

    #[test]
    fn tap_latches_and_any_second_dictation_down_stops_original_session() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(
            reducer.physical_down("F8", 0, Availability::Idle),
            Decision::Start
        );
        assert_eq!(reducer.physical_up("F8", 200), Decision::Latch);
        assert_eq!(
            reducer.physical_down("F9", 300, Availability::Active),
            Decision::Stop
        );
        assert_eq!(reducer.physical_up("F9", 350), Decision::Ignore);
    }

    #[test]
    fn busy_down_is_non_starting_and_its_up_is_consumed() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(
            reducer.physical_down("F8", 0, Availability::Busy),
            Decision::Busy
        );
        assert_eq!(reducer.physical_up("F8", 100), Decision::Ignore);
    }

    #[test]
    fn direct_controls_toggle_without_fabricating_physical_edges() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(reducer.direct_toggle(Availability::Idle), Decision::Start);
        assert_eq!(reducer.direct_toggle(Availability::Active), Decision::Stop);
        assert_eq!(reducer.direct_toggle(Availability::Busy), Decision::Busy);
    }

    #[test]
    fn escape_stops_only_the_active_original_session() {
        let mut reducer = AdaptiveReducer::default();
        assert_eq!(reducer.escape(Availability::Idle), Decision::Ignore);
        assert_eq!(reducer.escape(Availability::Active), Decision::Stop);
        assert_eq!(reducer.escape(Availability::Busy), Decision::Busy);
    }
}
