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

/// Now, in UTC, saying so: `2026-09-25T18:48:12Z`.
///
/// This used to be the wall clock at UTC+3 with no zone written - the way the C# wrote its
/// local time - and the client printed it as it was, so a reader at UTC+2 saw a version made
/// half an hour ago stamped half an hour in the future. In MongoDB it was worse: a stamp with
/// no zone is taken as UTC on the way in, so those instants are three hours late. They are
/// left as they are; from here on the instant is stored as what it is, and the client shows
/// it on the reader's own clock.
///
/// **With milliseconds, and never `.000`.** The old stamps had whole seconds, so in MongoDB
/// every one of them reads back as `…:SS.000Z` - which is how the client tells them apart and
/// reads them as the UTC+3 wall clock they really are. The C#'s stamps carry their own
/// fraction. A new stamp that happened to land on a whole second is nudged a millisecond so it
/// cannot be taken for an old one.
pub fn now_stamp() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let millis = if millis % 1000 == 0 { millis + 1 } else { millis };
    format!(
        "{}.{:03}Z",
        crate::api::stamp(millis / 1000).replace(' ', "T"),
        millis % 1000
    )
}
