# dodai-os-core

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/dodai-os/blob/main/LICENSE)

Part of [Dodai OS](https://github.com/Dodai-Tech/dodai-os).

Portable heart of Dodai OS: the capability model, touch gestures, time-driven animation and the `App` contract. `no_std`, no allocator, no `unsafe`. Everything else builds on it, and all of it is tested on the host and fuzzed.

* `Capability` and `CapabilitySet`: what a board offers and what an app needs; `runnable(board, apps)` picks the apps a board can run.
* `GestureRecognizer`: raw touch events in, tap, long press and swipe out.
* `Tween`, `ease_out_cubic`, `lerp`: integer animation driven by a clock.
* `App`, `Event`, `Effect`: an app is a state machine, events in and effects out.
