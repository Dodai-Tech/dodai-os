# hibari-kernel (experiment)

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os), a thin operating system for ESP32 boards, written in Rust.

A cooperative scheduler with fixed-size message queues and a capability table where capabilities can be delegated but never created. `no_std`, no allocation, no `unsafe`, fuzzed.

**Nothing uses it.** The ESP32 has no memory isolation to enforce, and the radio needs `esp-rtos` as its scheduler, so Hibari OS does not run on this kernel. It stays as an experiment.
