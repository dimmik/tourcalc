//! Tourcalc's server.
//!
//! Split into a library with a thin binary on top so that the tests can build a router and
//! talk to it directly, without a socket or a browser.

pub mod api;
pub mod auth;
pub mod config;
pub mod fields;
pub mod state;
pub mod store;
pub mod versions;
