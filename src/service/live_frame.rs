//! What the live tab is streamed each poll: the shared-memory snapshot plus everything derived
//! from it — the session's best lap and sectors, the ride-height record of the run, and whether
//! the session can be recorded by hand.
//!
//! Derived here rather than in the browser so every front end shows the same purple sector, the
//! same lowest corner and the same enabled Record button, from one copy of each rule.

use serde::Serialize;

use super::App;
use crate::game::shared_memory::LiveSessionData;

/// One poll's worth of the live tab.
#[derive(Serialize, Clone)]
pub struct LiveFrame {
    #[serde(flatten)]
    pub data: LiveSessionData,
    pub session_best: SessionBests,
    /// The run's ride-height extremes; `None` until a corner has been read on track.
    pub ride_seen: Option<RideSeen>,
    /// Whether the player is out on track — see [`on_track`]. The telemetry panel holds its last
    /// on-track reading while this is false.
    pub on_track: bool,
    /// Whether [`App::record_current_session`] would take this session — the Record button's
    /// state. An active career is not part of it: the click says so, which a greyed-out button
    /// could not.
    pub can_record: bool,
}

/// The fastest of everyone on track. `0.0` is "nobody has set one", the same unset convention
/// as the per-driver times it is taken from.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct SessionBests {
    /// What the gap-to-fastest column is measured from.
    pub lap: f32,
    /// What a sector is shown purple against.
    pub s1: f32,
    pub s2: f32,
    pub s3: f32,
}

/// Fastest positive value of `pick` across the field, or 0 when nobody has one.
fn fastest(data: &LiveSessionData, pick: impl Fn(&crate::game::shared_memory::ParticipantData) -> f32) -> f32 {
    data.participants
        .iter()
        .map(pick)
        .filter(|t| *t > 0.0)
        .fold(0.0, |best, t| if best == 0.0 || t < best { t } else { best })
}

pub fn session_bests(data: &LiveSessionData) -> SessionBests {
    SessionBests {
        lap: fastest(data, |p| p.fastest_lap_time),
        s1: fastest(data, |p| p.best_s1),
        s2: fastest(data, |p| p.best_s2),
        s3: fastest(data, |p| p.best_s3),
    }
}

/// The lowest and highest each corner has run at, FL / FR / RL / RR, in cm. `0.0` is "not read
/// yet" for that corner.
///
/// Per corner rather than one overall figure, because front and rear ride height are separate
/// setup values: the lowest corner says how much floor was left, and which end it was at says
/// which of the two can come down.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct RideSeen {
    pub min: [f32; 4],
    pub max: [f32; 4],
}

/// The ride-height record of the current run, shared by everything watching it.
///
/// One record for the app rather than one per viewer: two browsers watching the same run are
/// watching the same car, and a page reloaded mid-stint should not forget the stint.
#[derive(Default)]
pub(crate) struct RideRecord {
    /// Track, layout and car the readings belong to. Any of them changing is a new run — a figure
    /// carried across one would answer a question about Monza with a lap of Bathurst, or about
    /// this car with the last one's setup.
    run: String,
    seen: RideSeen,
}

impl RideRecord {
    /// Folds one poll in. Only readings taken out on track count — see [`on_track`] — and losing
    /// the game ends the run.
    fn fold(&mut self, data: &LiveSessionData) {
        if !data.connected {
            *self = Self::default();
            return;
        }
        if !on_track(data) {
            return;
        }
        let run = format!("{}|{}|{}", data.track_location, data.track_variation, data.car_name);
        if run != self.run {
            *self = Self {
                run,
                seen: RideSeen::default(),
            };
        }
        for (i, &v) in data.player_telemetry.ride_height.iter().enumerate() {
            // Zero is unset, not a car resting on its floor; it would otherwise always win.
            if v <= 0.0 {
                continue;
            }
            if self.seen.min[i] <= 0.0 || v < self.seen.min[i] {
                self.seen.min[i] = v;
            }
            if v > self.seen.max[i] {
                self.seen.max[i] = v;
            }
        }
    }

    fn seen(&self) -> Option<RideSeen> {
        self.seen.min.iter().any(|v| *v > 0.0).then_some(self.seen)
    }
}

/// Whether the player is out on track. Two signals, because neither alone catches every way off
/// it: `pit_mode` 0 is the only value that means on track, and it is what catches ESC → "Return
/// to pits", which teleports the car into the garage without driving down a pit lane; `in_pits`
/// flips as soon as the car crosses the pit entry, a moment before `pit_mode` does. A player who
/// cannot be found at all is in the garage too.
pub fn on_track(data: &LiveSessionData) -> bool {
    data.pit_mode == 0
        && data
            .participants
            .iter()
            .find(|p| p.is_player)
            .is_some_and(|p| !p.in_pits)
}

impl App {
    /// Reads the game and builds this poll's [`LiveFrame`], folding it into the run's
    /// ride-height record on the way.
    pub fn live_frame(&self) -> LiveFrame {
        self.frame_of(crate::game::shared_memory::read_live_session())
    }

    /// [`Self::live_frame`] for a snapshot already in hand.
    pub fn frame_of(&self, data: LiveSessionData) -> LiveFrame {
        let ride_seen = {
            let mut record = self.ride.lock().unwrap();
            record.fold(&data);
            record.seen()
        };
        LiveFrame {
            session_best: session_bests(&data),
            can_record: crate::game::recorder::recordable(&data).is_ok(),
            on_track: on_track(&data),
            ride_seen,
            data,
        }
    }

    /// Forgets the run's ride-height record — the live tab's Reset button, for a new stint on
    /// the same car and track after a setup change.
    pub fn reset_ride_record(&self) {
        *self.ride.lock().unwrap() = RideRecord::default();
    }
}
