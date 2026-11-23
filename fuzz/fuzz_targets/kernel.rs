#![no_main]
//! Random kernel calls never panic, and capabilities are never created from nothing: a task only
//! ever holds capabilities it was spawned with or that a holder delegated.

use hibari_device::{Capability, CapabilitySet};
use hibari_kernel::{Kernel, TaskId};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let mut k = Kernel::<4, 3>::new();
    let mut ever = CapabilitySet::EMPTY;
    for c in data.chunks_exact(4) {
        let (a, b) = (TaskId(c[1] % 6), TaskId(c[2] % 6));
        let cap = Capability::ALL[usize::from(c[3]) % Capability::ALL.len()];
        match c[0] % 6 {
            0 => {
                let caps = CapabilitySet::EMPTY.with(cap);
                if k.spawn(caps).is_ok() {
                    ever = ever.with(cap);
                }
            }
            1 => drop(k.kill(a)),
            2 => drop(k.send(a, b, u16::from(c[3]), 0)),
            3 => drop(k.recv(a)),
            4 => drop(k.delegate(a, b, cap)),
            _ => drop(k.next_ready()),
        }
        for i in 0..6 {
            if let Some(held) = k.caps(TaskId(i)) {
                assert!(ever.satisfies(held), "capability created from nothing");
            }
        }
    }
});
