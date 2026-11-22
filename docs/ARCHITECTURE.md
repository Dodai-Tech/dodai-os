# Dodai OS architecture

Dodai OS (土台, "foundation") is a small, efficient app platform for microcontroller boards:
RISC-V first (ESP32-C6, P4), then Xtensa (ESP32-S3), ARM Cortex-M and, experimentally, x86.
The goal is a microkernel with user-level services. We get there in stages, so there is always
something running on a real board.

## Layers

```
apps         weather, settings, clock ...      unprivileged state machines
shell        launcher, status bar, navigation  draws with dodai-core tokens
services     net, storage, ui, power           user-level servers, talk by IPC
kernel       scheduler, IPC, capabilities      tiny, static, no allocation
ports        riscv32, xtensa, cortex-m, x86_64 context switch, traps, MPU/PMP
backends     esp-idf (now), native no_std      implement the HAL traits
boards       one crate per device              pins, drivers, capability list
```

Apps never touch a backend. They see events in and effects out (`dodai-os-core`), and reach
hardware only through capabilities the shell grants.

## Crates

| Crate | Role | `no_std` |
|---|---|---|
| `dodai-os-core` | capabilities, touch gestures, the `App` contract | yes |
| `dodai-os-hal` | `Display`, `Touch`, `Backlight`, `Clock`, `Board` traits | yes |
| `dodai-os-shell` (planned) | launcher, home screen, navigation | yes + alloc |
| `dodai-os-kernel` (planned) | scheduler, IPC, capability table | yes |
| `dodai-os-port-<arch>` (planned) | architecture-specific code | yes |
| `dodai-os-<chip>` | firmware (ESP-IDF today), excluded from the workspace | no |
| `dodai-os-board-<name>` (planned) | one device: pins, drivers, capabilities | yes |

Design tokens and themes come from `dodai-core` (the `dodai-ui` repo); the OS never reuses
that name.

## Capabilities

A board declares a `CapabilitySet` (`Display`, `Touch`, `Wifi`, `Ble`, `Battery`, ...). An app
declares what it `requires`. The launcher lists `runnable(board, apps)`, so a headless C6 shows
no UI apps and a watch shows no app that needs a keyboard. At the kernel level a capability is
also the handle that grants access to a service (an unforgeable token), so isolation and app
filtering are the same mechanism.

## Staged plan

1. **Now: ESP-IDF backend.** Shell, apps and the HAL on the ESP32-S3 AMOLED 2.06. Radio, flash
   and drivers come from ESP-IDF. We get a working device and a stable app API.
2. **HAL and kernel API.** Apps and the shell compile against `dodai-os-core` and
   `dodai-os-hal` only. Host tests and a QEMU RISC-V target run in CI.
3. **Native backend.** `esp-hal` (`no_std`) plus `esp-wifi` for the radio, the Dodai kernel
   and the first port (RISC-V). Same apps, no change.
4. **More ports.** Xtensa, Cortex-M, then x86 (QEMU, framebuffer UI).

## Rules

* Core, HAL, kernel and shell stay `no_std` and `unsafe_code = "forbid"`; `unsafe` lives only
  in ports and board drivers, each block with a safety comment.
* The kernel allocates nothing at run time: static tables, static stacks, sizes known at build.
* Isolation uses the PMP (RISC-V) or MPU (Cortex-M) where it exists. Xtensa and the plain ESP32
  have none, so apps there are trusted, compiled-in code. This is stated, not hidden.
* No microcode or GPU code: rendering is software (Slint's software renderer), with optional
  2D accelerators such as the P4 PPA behind the `Display` trait.
* Radio code is Espressif's closed library. Networking is a driver service wrapping it.
* Every parser and state machine is unit-tested and fuzzed (`fuzz/`), and `just check-all`
  stays green.
