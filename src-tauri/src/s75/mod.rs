//! AULA S75 Pro support: a second keyboard family with its own protocol (see `protocol`).
//! Built from open-aula's notes; not yet tested on an S75 Pro.

pub mod device;
pub mod layout;
pub mod protocol;
pub mod screen;
#[cfg(windows)]
pub mod winlink;
#[cfg(windows)]
pub mod app;
