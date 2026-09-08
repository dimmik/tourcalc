//! Tourcalc's domain and arithmetic.
//!
//! This crate knows nothing about HTTP, databases, browsers or the clock. It takes data
//! and returns data, which is what lets it compile both to a native server binary and to
//! `wasm32-unknown-unknown` for the offline client - one implementation of the money, used
//! by the API, the text pages, the bot and the browser alike.
//!
//! That is also the reason the dependency list is two entries long: **anything that lands
//! here lands in the browser.**

pub mod calc;
pub mod domain;
pub mod ids;
pub mod money;

pub use calc::{calculate, suggest_settlement, Balances, Options, PersonBalance, Transfer};
pub use domain::{Currency, Kind, Person, Spending, Split, Tour};
pub use ids::{CurrencyId, PersonId, SpendingId, TourId};
pub use money::Cents;
