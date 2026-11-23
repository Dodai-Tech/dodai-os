#![no_main]
//! Random touch streams never panic, and after an `Up` the recognizer is idle.

use hibari_os_core::{GestureRecognizer, Phase, Point, TouchEvent};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut g = GestureRecognizer::default();
    let mut t = 0u32;
    for c in data.chunks_exact(6) {
        t = t.wrapping_add(u32::from(c[1]));
        let phase = match c[0] % 3 {
            0 => Phase::Down,
            1 => Phase::Move,
            _ => Phase::Up,
        };
        let point = Point { x: i16::from_le_bytes([c[2], c[3]]), y: i16::from_le_bytes([c[4], c[5]]) };
        g.feed(TouchEvent { phase, point, t_ms: t });
        g.tick(t);
        if phase == Phase::Up {
            assert!(!g.is_active());
        }
    }
});
