//! The app contract: what an app declares, what it receives and what it answers.

use crate::capability::CapabilitySet;
use crate::touch::Gesture;

/// Static description of an app, shown by the launcher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppInfo {
    /// Stable machine id, for example `"weather"`.
    pub id: &'static str,
    /// Name shown to the user.
    pub name: &'static str,
    /// Capabilities the board must offer for the app to run.
    pub requires: CapabilitySet,
}

/// Something the shell tells an app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The app came to the foreground.
    Start,
    /// The app is leaving the foreground.
    Stop,
    /// Time passed; `now_ms` is monotonic.
    Tick {
        /// Monotonic time in milliseconds.
        now_ms: u32,
    },
    /// The user made a gesture.
    Gesture(Gesture),
}

/// What an app asks the shell to do after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Nothing.
    None,
    /// The view changed; draw again.
    Redraw,
    /// The app is done; go back to the launcher.
    Quit,
}

/// An app. It is a state machine: events in, effects out, no direct hardware access
/// (services reach it through capabilities the shell hands over).
pub trait App {
    /// The app's description.
    fn info(&self) -> &AppInfo;

    /// Handles one event.
    fn handle(&mut self, event: Event) -> Effect;
}

/// The apps in `apps` that `board` can run, in order.
pub fn runnable(board: CapabilitySet, apps: &[AppInfo]) -> impl Iterator<Item = &AppInfo> {
    apps.iter().filter(move |a| board.satisfies(a.requires))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::Capability::{Ble, Display, Touch};

    const APPS: [AppInfo; 3] = [
        AppInfo {
            id: "clock",
            name: "Clock",
            requires: CapabilitySet::of(&[Display]),
        },
        AppInfo {
            id: "pair",
            name: "Pairing",
            requires: CapabilitySet::of(&[Display, Touch, Ble]),
        },
        AppInfo {
            id: "daemon",
            name: "Daemon",
            requires: CapabilitySet::EMPTY,
        },
    ];

    #[test]
    fn headless_board_only_runs_services() {
        let ids: Vec<_> = runnable(CapabilitySet::EMPTY, &APPS)
            .map(|a| a.id)
            .collect();
        assert_eq!(ids, ["daemon"]);
    }

    #[test]
    fn board_gets_the_apps_it_satisfies() {
        let board = CapabilitySet::of(&[Display, Touch]);
        let ids: Vec<_> = runnable(board, &APPS).map(|a| a.id).collect();
        assert_eq!(ids, ["clock", "daemon"]);
    }

    struct Counter(AppInfo, u32);
    impl App for Counter {
        fn info(&self) -> &AppInfo {
            &self.0
        }
        fn handle(&mut self, event: Event) -> Effect {
            match event {
                Event::Tick { .. } => {
                    self.1 += 1;
                    Effect::Redraw
                }
                Event::Stop => Effect::Quit,
                _ => Effect::None,
            }
        }
    }

    #[test]
    fn app_is_a_state_machine() {
        let mut app = Counter(APPS[0], 0);
        assert_eq!(app.handle(Event::Tick { now_ms: 1 }), Effect::Redraw);
        assert_eq!(app.handle(Event::Start), Effect::None);
        assert_eq!(app.handle(Event::Stop), Effect::Quit);
        assert_eq!(app.1, 1);
        assert_eq!(app.info().id, "clock");
    }
}
