//! Dodai OS core: the portable, `no_std` heart of the platform.
//!
//! It holds only pure logic that runs on any CPU and on the host: the capability model
//! ([`capability`]), touch gestures ([`touch`]) and the app contract ([`app`]). Hardware
//! comes in through `dodai-os-hal`; ESP-IDF and the native kernel are backends below it.
#![cfg_attr(not(test), no_std)]

pub mod app;
pub mod capability;
pub mod touch;

pub use app::{App, AppInfo, Effect, Event, runnable};
pub use capability::{Capability, CapabilitySet};
pub use fusuma_anim as anim;
pub use touch::{Direction, Gesture, GestureConfig, GestureRecognizer, Phase, Point, TouchEvent};
