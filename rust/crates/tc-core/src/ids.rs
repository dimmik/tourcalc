//! Identifiers.
//!
//! In the C# app every id is a `string`: `Tour.Id`, `Person.GUID`, `Spending.FromGuid`,
//! `Person.ParentId`, `Person.GroupId`. Seven different meanings sharing one type, and the
//! only thing standing between them is attention.
//!
//! A newtype costs nothing at runtime - after compilation `PersonId` *is* a `String` - but
//! passing one where the other is expected stops being a bug you find in production and
//! becomes a bug you cannot write.

use serde::{Deserialize, Serialize};

/// Declares a wrapper around a string that behaves like a string but is not one.
///
/// `#[serde(transparent)]` keeps it a plain string on the wire, so the existing JSON in
/// Mongo and in everyone's localStorage still reads.
macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Identifies a participant within one tour.
    PersonId
);
string_id!(
    /// Identifies a spending within one tour.
    SpendingId
);
string_id!(
    /// Identifies a tour.
    TourId
);
string_id!(
    /// Identifies a currency ("RUB", "EUR"); the name doubles as the id in the old data.
    CurrencyId
);
