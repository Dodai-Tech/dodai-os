//! What a Hibari OS device is: the capabilities a board offers, its screen, its name and how to build
//! firmware for it.
//!
//! The same description is used by the firmware (as constants generated from `board.toml` at build
//! time), by the host preview (which loads the same file) and by the build scripts, so a device is
//! written down once. `no_std`, no allocator; the `toml` feature adds loading from TOML text and
//! generating the constants, for tools and build scripts.
#![no_std]

#[cfg(any(feature = "toml", test))]
extern crate std;

mod capability;
mod profile;
mod render;
#[cfg(feature = "toml")]
mod text;

pub use capability::{Capability, CapabilitySet};
pub use profile::{DeviceProfile, DisplayInfo, Firmware, ProfileError, Shape};
pub use render::{
    Backend, Choice, Fit, Hardware, Machine, Plan, RAM_SHARE_PERCENT, Reason, Render, automatic,
    budget, check, choose, fits, machine, nearest, plan,
};
