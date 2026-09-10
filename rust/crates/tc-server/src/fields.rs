//! The parts of a stored tour that are about storage rather than about money.
//!
//! `StateGUID`, `AccessCodeMD5`, `IsVersion` and the rest are not modelled by `tc-core` -
//! they say who may look and which copy this is, which is the server's business and not the
//! arithmetic's - so they ride through in [`tc_core::Extras`] and are read from there.
//!
//! Every read is case-insensitive, and that is not fussiness: three of the eight tours in
//! the seed file spell every field in camelCase. Looking only for the capitalised spelling
//! made those tours invisible to everybody, which showed up as "two tours missing from the
//! list" and as an error nowhere.

use tc_core::Tour;

// The names and the reading live in `tc_core::extras`, because the browser client needs
// exactly the same ones and two copies of "how is IsArchived spelled" is one too many.
pub use tc_core::extras::{
    ACCESS_CODE, ARCHIVED, CREATED_AT, FINALIZING, INTERNAL_VERSION_COMMENT, IS_VERSION, STATE,
    VERSIONED_AT, VERSION_COMMENT, VERSION_FOR,
};

pub fn str_of(tour: &Tour, key: &str) -> String {
    tc_core::extras::str_of(&tour.extras, key)
}

pub fn bool_of(tour: &Tour, key: &str) -> bool {
    tc_core::extras::bool_of(&tour.extras, key)
}

pub fn set(tour: &mut Tour, key: &str, value: serde_json::Value) {
    tc_core::extras::set(&mut tour.extras, key, value)
}

pub fn remove(tour: &mut Tour, key: &str) {
    tc_core::extras::remove(&mut tour.extras, key)
}

/// Whether this record is a kept copy of an earlier state rather than a tour in its own
/// right. Versions live in the same collection and are filtered out of every list.
pub fn is_version(tour: &Tour) -> bool {
    bool_of(tour, IS_VERSION)
}

/// Which pile of tours this one belongs to.
pub fn access_code(tour: &Tour) -> String {
    str_of(tour, ACCESS_CODE)
}

/// A timestamp in the shape the C# writes into these fields.
pub fn now_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        // The C# writes local time, and the deployment runs at UTC+3. A timestamp that read
        // differently would look like a bug to anybody comparing two records.
        + 3 * 3600;
    crate::api::stamp(secs)
}
