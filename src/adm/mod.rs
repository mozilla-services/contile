//! ADM partner integration
//!
//! This module handles most of the ADM fetch, validation, and management
//! for tile data. ADM provides a list of partners, along with a set of
//! tile information (e.g. the name of the partner, various URLs, etc.)
//! We only allow a known set of partners, and validate that the tile info
//! offered matches expected values.

mod filter;
mod settings;
mod tiles;

pub use filter::AdmFilter;
pub(crate) use settings::{AdmAdvertiserFilterSettings, AdmSettings, DEFAULT};
pub use tiles::{get_tiles, TileResponse};
