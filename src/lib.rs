//! Platform-independent foundation. Native terminal integration is gated by milestone 0.
pub const GHOSTTY_REVISION: &str = "291fd050b9e9c293c8adb6a140742649b7e42d3a";

#[cfg(all(feature = "pty", unix))]
pub mod process;
