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

pub const STATE: &str = "StateGUID";
pub const ACCESS_CODE: &str = "AccessCodeMD5";
pub const IS_VERSION: &str = "IsVersion";
pub const VERSION_FOR: &str = "VersionFor_Id";
pub const VERSIONED_AT: &str = "DateVersioned";
pub const VERSION_COMMENT: &str = "VersionComment";
/// Set by a client asking for a particular comment on the version this save creates. The C#
/// uses it for "Tour Restored to ..."; it is never stored on the tour itself.
pub const INTERNAL_VERSION_COMMENT: &str = "InternalVersionComment";
pub const CREATED_AT: &str = "DateCreated";
pub const ARCHIVED: &str = "IsArchived";
pub const FINALIZING: &str = "IsFinalizing";

pub fn str_of(tour: &Tour, key: &str) -> String {
    tour.extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_str())
        .unwrap_or("")
        .to_owned()
}

pub fn bool_of(tour: &Tour, key: &str) -> bool {
    tour.extras
        .0
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .and_then(|(_, v)| v.as_bool())
        .unwrap_or(false)
}

pub fn set(tour: &mut Tour, key: &str, value: serde_json::Value) {
    // Replace whatever spelling is already there, so a camelCase tour does not end up with
    // both `stateGUID` and `StateGUID`.
    let existing: Option<String> = tour
        .extras
        .0
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned();
    let key = existing.unwrap_or_else(|| key.to_owned());
    tour.extras.0.insert(key, value);
}

pub fn remove(tour: &mut Tour, key: &str) {
    let existing: Option<String> = tour
        .extras
        .0
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned();
    if let Some(k) = existing {
        tour.extras.0.remove(&k);
    }
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
