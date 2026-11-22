//! The public API as a user of the crate sees it.

use dodai_os_core::{
    AppInfo, Capability, CapabilitySet, Gesture, GestureRecognizer, Phase, Point, TouchEvent,
    runnable,
};

#[test]
fn board_filters_apps_and_gestures_work_together() {
    let board = CapabilitySet::of(&[Capability::Display, Capability::Touch]);
    let apps = [AppInfo {
        id: "weather",
        name: "Weather",
        requires: CapabilitySet::of(&[Capability::Display, Capability::Wifi]),
    }];
    assert_eq!(runnable(board, &apps).count(), 0);

    let mut g = GestureRecognizer::default();
    let at = Point { x: 10, y: 10 };
    g.feed(TouchEvent {
        phase: Phase::Down,
        point: at,
        t_ms: 0,
    });
    assert_eq!(
        g.feed(TouchEvent {
            phase: Phase::Up,
            point: at,
            t_ms: 50
        }),
        Some(Gesture::Tap(at))
    );
}
