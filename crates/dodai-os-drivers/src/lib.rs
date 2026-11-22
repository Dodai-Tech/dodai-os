//! Peripheral drivers for Dodai OS.
//!
//! Each driver is generic over `embedded-hal` traits (or a tiny bus trait where `embedded-hal`
//! has none, like QSPI), so it runs on any chip and is tested on the host with fake buses. The
//! board crates supply the real buses and pins. Drivers implement the `dodai-os-hal` traits.
#![cfg_attr(not(test), no_std)]

pub mod co5300;
pub mod ft3168;
