//! The Hibari OS launcher: the app manifest ([`app`]), the logic that pages and navigates ([`model`]),
//! an interim view that draws it through Fusuma's `Renderer` ([`view`]) and a frame buffer that tracks
//! which rows changed ([`framebuf`]). `no_std`, no allocator.
#![cfg_attr(not(test), no_std)]

pub mod app;
pub mod framebuf;
pub mod model;
pub mod view;

pub use app::{AppInfo, runnable};
pub use framebuf::FrameBuf;
pub use model::{Launcher, Output, Screen, Shell};
pub use view::{SLIDE_MS, Theme, Ui};
