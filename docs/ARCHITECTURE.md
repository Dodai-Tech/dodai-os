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
| `dodai-os-shell-model` | launcher paging, hit testing, navigation (no UI toolkit) | yes |
| `dodai-os-shell` (planned) | Slint view on the shell model | yes + alloc |
| `dodai-os-kernel` | cooperative scheduler, message queues, capability table | yes |
| `dodai-os-host` | hosted backend: device profiles, host clock and display | no |
| `dodai-os-port-<arch>` (planned) | architecture-specific code | yes |
| `dodai-os-<chip>` | firmware (ESP-IDF today), excluded from the workspace | no |
| `dodai-os-board-<name>` (planned) | one device: pins, drivers, capabilities | yes |

Design tokens and themes come from `dodai-core` (the `dodai-ui` repo); the OS never reuses
that name.

## Products

| Product | What it is | Built on |
|---|---|---|
| **Dodai OS** | Own microkernel and app platform, staged by hardware: RISC-V ESPs first, then Xtensa, ARM, x86, open or unlockable phones, tablets and watches | own kernel (ESP-IDF while bootstrapping) |
| **Dodai DE** | Desktop and mobile shell in Rust: Wayland compositor, panel, launcher, settings | Linux (probably Smithay) |
| **Dodai Linux** | Arch-based distribution that ships Dodai DE: installer, package repo, `archiso` images | Arch Linux |

Shared by all three: `dodai-core` and `dodai-ui` (tokens, themes, widgets), `dodai-os-core`
(`no_std`: capabilities, gestures, the `App` contract), the shell model (launcher, navigation,
settings pages, notifications; UI-toolkit independent) and the apps. Not shared: the HAL
(devices only; the DE gets input and display from libinput, DRM and Wayland), the kernel, ports
and board crates (Dodai OS only), the compositor, session, packaging and installer (DE and Linux).

Rule: a shared crate has no platform dependency. Each product supplies a thin adapter.
"Any hardware" means any hardware that has a port and drivers: per device, not automatic.
Desktop-class hardware (GPU, Wi-Fi, USB, audio) is the long tail for the native kernel, so
Dodai Linux covers desktops and laptops first.

## Run modes

One set of apps, shell, capabilities and HAL traits; only the layer under the HAL changes.

| Mode | What it is | Targets |
|---|---|---|
| **Standalone, own kernel** | Dodai is the OS and boots on the hardware | MCUs (ESP32, RP, nRF, STM32), QEMU, Raspberry Pi, x86 PCs, open or unlockable phones and wearables |
| **On Linux** | Dodai is the userspace shell on a minimal Linux kernel; Linux supplies drivers, GPU and radio | devices with mainline or vendor Linux, including unlocked phones |
| **Hosted** | Dodai is an app inside another OS (`dodai-os-host`, a window plus host input and clock) | Linux, macOS, Windows, Android, iOS; also the development preview |

## Device feasibility (standalone)

The bootloader is rarely the problem: many Android phones unlock and boot a custom image. The
blocker is undocumented hardware: closed GPU, modem, Wi-Fi/BT firmware, camera and audio DSPs.

| Device class | Own kernel | On Linux |
|---|---|---|
| MCUs, wearables and panels with Rust HALs | yes, first | n/a |
| Raspberry Pi and ARM SBCs | yes | yes |
| x86 PC (UEFI framebuffer, VirtIO, ACPI, PCI, USB) | yes, hard | yes |
| Unlocked phones with good mainline Linux support | display, touch and storage first; radio is the hard part | realistic |
| Unlocked phones with poor mainline support | mostly no | vendor kernel |
| iPhone, Apple Watch, locked phones | no | no |

The first phone target is the best-supported unlocked device the project owner has (check the
postmarketOS device tiers). QEMU (`virt` for RISC-V and ARM, `q35` for x86) is the CI target for
every port.

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
5. **Hosted and Linux modes**, then phones: see `ROADMAP.md`.

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
