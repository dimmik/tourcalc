//! Tours kept in a GitHub repository - a public one - encrypted (`StorageType=GitHub`).
//!
//! The plan, the decisions and what an outsider can still see: `rust/github-store.md`.

pub mod remote;
pub mod seal;
pub mod store;

pub use store::{GitHubStore, GitHubSubscriptions, Settings};
