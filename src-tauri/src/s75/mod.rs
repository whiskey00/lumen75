//! AULA S75 Pro support: a second keyboard family with its own protocol (see `protocol`).
//! Built from open-aula's notes; tested on an S75 Pro (2026-10-22).

pub mod device;
pub mod layout;
pub mod protocol;
pub mod screen;
#[cfg(windows)]
pub mod winlink;
#[cfg(windows)]
pub mod app;
