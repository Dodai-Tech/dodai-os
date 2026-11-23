//! The app manifest: what an app is called and what a device must offer to run it. The launcher lists
//! only the apps a device can run.

use hibari_device::CapabilitySet;

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

/// The apps in `apps` that a device with the capabilities `board` can run, in order.
pub fn runnable(board: CapabilitySet, apps: &[AppInfo]) -> impl Iterator<Item = &AppInfo> {
    apps.iter().filter(move |a| board.satisfies(a.requires))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hibari_device::Capability::{Ble, Display, Touch};

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
}
