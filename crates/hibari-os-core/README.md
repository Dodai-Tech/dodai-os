# hibari-os-core

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os).

Portable heart of Hibari OS: the capability model, touch gestures, time-driven animation and the `App` contract. `no_std`, no allocator, no `unsafe`. Everything else builds on it, and all of it is tested on the host and fuzzed.

* `Capability` and `CapabilitySet`: what a board offers and what an app needs; `runnable(board, apps)` picks the apps a board can run.
* `GestureRecognizer`: raw touch events in, tap, long press and swipe out.
* `anim`: [`fusuma-anim`](https://crates.io/crates/fusuma-anim) re-exported: easing, tweens, keyframes, springs and fling decay in fixed point, driven by a clock.
* `App`, `Event`, `Effect`: an app is a state machine, events in and effects out.
