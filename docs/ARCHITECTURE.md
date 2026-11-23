# Hibari OS architecture

Hibari OS (土台, "foundation") is a small, efficient app platform for microcontroller boards:
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

Apps never touch a backend. They see events in and effects out (`hibari-os-core`), and reach
hardware only through capabilities the shell grants.

## Crates

| Crate | Role | `no_std` |
|---|---|---|
| `hibari-os-core` | capabilities, touch gestures, the `App` contract | yes |
| `hibari-os-hal` | `Display`, `Touch`, `Backlight`, `Clock`, `Board` traits | yes |
| `hibari-os-shell-model` | launcher paging, hit testing, navigation (no UI toolkit) | yes |
| `hibari-os-shell` | view: status bar, app grid, app page, drawn with `embedded-graphics` on any RGB565 target | yes |
| `hibari-os-kernel` | cooperative scheduler, message queues, capability table | yes |
| `hibari-os-host` | hosted backend: device profiles, host clock and display | no |
| `hibari-os-port-<arch>` (planned) | architecture-specific code | yes |
| `hibari-os-<chip>` | firmware (ESP-IDF today), excluded from the workspace | no |
| `hibari-os-board-<name>` (planned) | one device: pins, drivers, capabilities | yes |

Design tokens and themes come from `dodai-core` (in the `dodai-ui` repository); OS crates are
`hibari-os-*`, so the names never collide.

## Products

| Product | What it is | Built on |
|---|---|---|
| **Hibari OS** | Own microkernel and app platform, staged by hardware: RISC-V ESPs first, then Xtensa, ARM, x86, open or unlockable phones, tablets and watches | own kernel (ESP-IDF while bootstrapping) |
| **Kawara** (the Dodai desktop environment) | Desktop and mobile shell in Rust: Wayland compositor, panel, launcher, settings | Linux (probably Smithay) |
| **Dodai Linux** | Arch-based distribution that ships Kawara: installer, package repo, `archiso` images | Arch Linux |

Shared by all three: `dodai-core` and `dodai-ui` (tokens, themes, widgets), `hibari-os-core`
(`no_std`: capabilities, gestures, the `App` contract), the shell model (launcher, navigation,
settings pages, notifications; UI-toolkit independent) and the apps. Not shared: the HAL
(devices only; the DE gets input and display from libinput, DRM and Wayland), the kernel, ports
and board crates (Hibari OS only), the compositor, session, packaging and installer (DE and Linux).

Rule: a shared crate has no platform dependency. Each product supplies a thin adapter.
"Any hardware" means any hardware that has a port and drivers: per device, not automatic.
Desktop-class hardware (GPU, Wi-Fi, USB, audio) is the long tail for the native kernel, so
Dodai Linux covers desktops and laptops first.

## The Dodai family

Names follow a Japanese house. Dodai (土台, foundation) is the family and the design system; Hibari (雲雀, skylark) is the thin operating system for ESP32 boards, and each layer above has its own
name, so each can be used without the others.

| House part | Product | Name | Notes |
|---|---|---|---|
| Foundation 土台 | OS with its own microkernel | **Hibari OS** (`hibari-os-*`) | this repo |
| Sliding doors 襖 | UI toolkit and its renderer | **Fusuma** | OS-agnostic. Renderer: `fusuma-render` (the trait), `-embedded` (microcontrollers), `-vello-cpu` and later `-vello` (GPU); animation: `fusuma-anim`; widgets, layout, text, effects and shaders (`fusuma-*`) are planned. One repo, `fusuma`; apps depend on one `fusuma` crate and the backend is chosen by target. See its `docs/ROADMAP.md`. |
| Finish | design system | **Dodai UI** (`dodai-*`) | tokens, four designs, one adapter crate per toolkit, including `dodai-fusuma` |
| Roof tiles 瓦 | desktop and mobile environment | **Kawara** | Linux, Wayland. Planned. |
| Shop curtain 暖簾 | app store | **Noren** | front end over Flatpak and pacman; a store for Hibari OS is research. Planned. |
| The house | Arch-based distribution | **Dodai Linux** | ships Kawara and Noren. Planned. |

Dependencies point one way: Hibari OS and Kawara use Fusuma, the toolkit crates use `fusuma-render`, and
`fusuma-render*` knows nothing above it; a CI rule in the Fusuma repo enforces that. `dodai-fusuma` uses Fusuma and the design tokens; Fusuma never imports
`dodai-*`.

Naming rules:
* Design system (repository `dodai-ui`): `dodai-<toolkit>` is an adapter (`dodai-slint`, `dodai-iced`,
  `dodai-fusuma`); `dodai-<capability>` is toolkit-neutral (`dodai-core`, `dodai-icons`, `dodai-shaders`).
* `hibari-os-<role>` is the OS; `-core` means the portable, UI-free heart of that product.
* Logic shared by Hibari OS and Kawara (the launcher model) gets a neutral name when Kawara starts
  (`hibari-os-shell-model` becomes a neutral `shell-model`), so neither product depends on the other.

## Rendering and platforms: native Rust, no web technology

Everything draws through `fusuma-render`:

| Situation | Backend |
|---|---|
| Microcontroller with a pixel display | `fusuma-render-embedded` (our integer rasteriser, `no_std`) |
| Desktop, phone, watch, with a GPU | `fusuma-render-vello` (`vello_gpu`), planned |
| The same without a usable GPU; screenshots and tests | `fusuma-render-vello-cpu` (`vello_cpu`) |
| Terminal, serial console, character displays | `fusuma-platform-tui`, planned: a semantic front end on ratatui, not a pixel backend |

The two Vello backends share one implementation of the `Renderer` trait. There is no webview, HTML
or CSS in Fusuma, and the C++ Skia is not used (the `tiny-skia` backend was dropped). Animation
(easing, tweens, keyframes, springs, fling decay) is built in as `fusuma-anim`, in fixed-point maths
so that it runs the same without an FPU. Effects (shadow, blur, gradients) and shaders are optional
capabilities that a backend reports; widgets fall back to a plain fill without them.

Fusuma reaches each target through a small platform crate:

| Crate | Target | Notes |
|---|---|---|
| `fusuma-platform-mcu` | microcontroller displays | display and touch drivers, as in Hibari OS today |
| `fusuma-platform-desktop` | Linux, macOS, Windows | `winit` window, pixel buffer or GPU surface |
| `fusuma-platform-android`, `fusuma-platform-ios` | phones and tablets | native windows and touch; no web wrapper |
| Kawara's Wayland platform | Linux desktop and mobile | native, in the Kawara repo |
| `fusuma-platform-web` | browsers, later and optional | Rust compiled to WebAssembly, drawn into a canvas; no DOM widgets |

Because Fusuma draws everything itself it must also provide text input with an input method
(`winit` supplies the events), accessibility (`AccessKit`) and a Rust text stack (`parley` and
`skrifa`, or `cosmic-text` and `swash`; to be chosen). These are the costly parts on phones and the
web, and they are listed as their own roadmap items.

## Run modes

One set of apps, shell, capabilities and HAL traits; only the layer under the HAL changes.

| Mode | What it is | Targets |
|---|---|---|
| **Standalone** | Hibari is the OS and boots on the hardware | MCUs (ESP32, RP, nRF, STM32), QEMU, Raspberry Pi, x86 PCs, open or unlockable phones and wearables |
| **On Linux** | Hibari is the userspace shell on a minimal Linux kernel; Linux supplies drivers, GPU and radio | devices with mainline or vendor Linux, including unlocked phones |
| **Hosted** | Hibari is an app inside another OS (`hibari-os-host`, a window plus host input and clock) | Linux, macOS, Windows, Android, iOS; also the development preview |

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
2. **HAL and kernel API.** Apps and the shell compile against `hibari-os-core` and
   `hibari-os-hal` only. Host tests and a QEMU RISC-V target run in CI.
3. **Native backend.** `esp-hal` (`no_std`) plus `esp-wifi` for the radio, the Hibari runtime
   and the first port (RISC-V). Same apps, no change.
4. **More ports.** Xtensa, Cortex-M, then x86 (QEMU, framebuffer UI).
5. **Hosted and Linux modes**, then phones: see `ROADMAP.md`.

## Rules

* Core, HAL, kernel and shell stay `no_std` and `unsafe_code = "forbid"`; `unsafe` lives only
  in ports and board drivers, each block with a safety comment.
* The kernel allocates nothing at run time: static tables, static stacks, sizes known at build.
* Isolation uses the PMP (RISC-V) or MPU (Cortex-M) where it exists. Xtensa and the plain ESP32
  have none, so apps there are trusted, compiled-in code. This is stated, not hidden.
* No microcode or GPU code: rendering is software (through `fusuma-render`: the integer renderer on microcontrollers, Vello elsewhere; Slint stays an optional view because its GPL or commercial license does not fit an MIT-only dependency policy), with optional
  2D accelerators such as the P4 PPA behind the `Display` trait.
* Radio code is Espressif's closed library. Networking is a driver service wrapping it.
* Every parser and state machine is unit-tested and fuzzed (`fuzz/`), and `just check-all`
  stays green.
