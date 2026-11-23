# hibari-os-kernel

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/Dodai-Tech/hibari-os/blob/main/LICENSE)

Part of [Hibari OS](https://github.com/Dodai-Tech/hibari-os).

The microkernel, first stage: a static cooperative scheduler, fixed-size message queues and a capability table where capabilities can be delegated but never created. `no_std`, no allocation, no `unsafe`. Preemption and the architecture ports come later behind the `Port` trait.
