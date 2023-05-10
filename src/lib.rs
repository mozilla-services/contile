#![warn(rust_2018_idioms)]
#![allow(clippy::try_err)]
#![forbid(unsafe_code)]

//! Context Tile service
//!
//! Tiles are the links displayed on a "Firefox Home" page (displayed as
//! a new tab or default open page.) Context tiles are sponsored tiles
//! that can be offered. These tiles are provided by an advertising
//! partner (ADM). Contile provides a level of additional privacy by
//! disclosing only the minimal user info required, and providing a
//! caching system.

#[macro_use]
extern crate slog_scope;

pub mod adm;
#[macro_use]
pub mod logging;
pub mod error;
pub mod metrics;
pub mod server;
pub mod settings;
pub mod tags;
pub mod web;

/// Create the version string (e.g. "contile/1.0.0") with the given separator.
/// It expects an environment variable `CONTILE_VERSION` as the version and
/// falls back to `CARGO_PKG_VERSION` if it's not present.
pub fn create_app_version(separator: &str) -> String {
    let app = env!("CARGO_PKG_NAME");
    let version = option_env!("CONTILE_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));

    format!("{app}{separator}{version}")
}

#[cfg(test)]
mod tests {
    use crate::create_app_version;

    #[test]
    fn test_create_app_version_fallback() {
        assert_eq!(
            create_app_version("/"),
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
        );
    }
}
