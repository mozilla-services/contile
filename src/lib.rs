#![warn(rust_2018_idioms)]
#![allow(clippy::try_err)]

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
