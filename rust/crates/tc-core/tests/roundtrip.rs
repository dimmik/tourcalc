//! Reading a tour and writing it back must not change it.
//!
//! This is the other half of the compatibility requirement. `golden.rs` checks that the
//! arithmetic agrees with C#; this checks that a tour which passes through this crate is
//! still the same tour afterwards - because the first time the rewritten app saves, it
//! overwrites somebody's real trip.

use serde_json::Value;
use std::path::{Path, PathBuf};
use tc_core::Tour;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn tour_files() -> Vec<(String, String)> {
    let dir = fixtures_dir();
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("fixtures directory")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let stem = n.strip_suffix(".tour.json")?.to_owned();
            let body = std::fs::read_to_string(e.path()).ok()?;
            Some((stem, body))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!out.is_empty(), "no fixtures in {}", dir.display());
    out
}

/// Parse, write, parse again: the tour that comes back is the tour that went in.
///
/// Compares the domain values rather than the text, so it does not care about key order or
/// whitespace - only about meaning.
#[test]
fn domain_survives_a_round_trip() {
    for (name, json) in tour_files() {
        let once = Tour::from_json(&json).unwrap_or_else(|e| panic!("[{name}] parse: {e}"));
        let written = once
            .to_json()
            .unwrap_or_else(|e| panic!("[{name}] write: {e}"));
        let twice = Tour::from_json(&written).unwrap_or_else(|e| panic!("[{name}] reparse: {e}"));
        assert_eq!(once, twice, "[{name}] tour changed by a round trip");
    }
}

/// Every field of the original JSON is still there, with the same value.
///
/// The strict version of the same idea: whatever the C# app wrote, we write back. Fields
/// this crate does not model are carried through `Extras` rather than dropped, so the check
/// can be "nothing is missing" rather than "nothing important is missing".
#[test]
fn no_field_is_lost() {
    for (name, json) in tour_files() {
        let tour = Tour::from_json(&json).unwrap();
        let before: Value = serde_json::from_str(&json).unwrap();
        let after: Value = serde_json::from_str(&tour.to_json().unwrap()).unwrap();

        let mut missing = Vec::new();
        compare(&before, &after, String::new(), &mut missing);

        assert!(
            missing.is_empty(),
            "[{name}] {} field(s) lost or changed:\n  {}",
            missing.len(),
            missing.join("\n  ")
        );
    }
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        _ => false,
    }
}

/// Walks the original and reports anything the rewritten copy does not have, or has
/// differently. Extra fields in the copy are fine; missing ones are not.
fn compare(before: &Value, after: &Value, path: String, out: &mut Vec<String>) {
    // The stored data spells "nobody" both ways in different tours, and everything that
    // reads it uses IsNullOrWhiteSpace, so the two are the same value written differently.
    if path.ends_with(".ParentId") && is_blank(before) && is_blank(after) {
        return;
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => compare(va, vb, format!("{path}.{k}"), out),
                    None => out.push(format!("{path}.{k} is missing")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            if a.len() != b.len() {
                out.push(format!("{path} has {} items, was {}", b.len(), a.len()));
                return;
            }
            for (i, (va, vb)) in a.iter().zip(b).enumerate() {
                compare(va, vb, format!("{path}[{i}]"), out);
            }
        }
        (a, b) if a != b => out.push(format!("{path}: {a} became {b}")),
        _ => {}
    }
}
