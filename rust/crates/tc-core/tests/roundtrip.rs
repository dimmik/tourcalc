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
    if is_blank(before) && is_blank(after) {
        return;
    }
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, va) in a {
                match b.get(k) {
                    Some(vb) => compare(va, vb, format!("{path}.{k}"), out),
                    // A field that held nothing and is now absent still holds nothing:
                    // everything that reads these treats null, "" and missing alike.
                    None if is_blank(va) => {}
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

/// A tour written in camelCase reads the same as one written in PascalCase.
///
/// Both spellings are in the seed file the server boots from: Newtonsoft never minded, so
/// nobody noticed. serde does mind, and three tours silently came back empty until the
/// field names were normalised.
#[test]
fn camel_case_reads_too() {
    let pascal = r#"{
        "Id": "t1", "Name": "Trip",
        "Persons": [
            {"GUID": "a", "Name": "Ann", "Weight": 100},
            {"GUID": "b", "Name": "Bob", "Weight": 100}
        ],
        "Spendings": [
            {"GUID": "s1", "FromGuid": "a", "AmountInCents": 1000, "ToAll": true, "Type": "food"}
        ]
    }"#;
    let camel = r#"{
        "id": "t1", "name": "Trip",
        "persons": [
            {"guid": "a", "name": "Ann", "weight": 100},
            {"guid": "b", "name": "Bob", "weight": 100}
        ],
        "spendings": [
            {"guid": "s1", "fromGuid": "a", "amountInCents": 1000, "toAll": true, "type": "food"}
        ]
    }"#;

    let a = Tour::from_json(pascal).expect("PascalCase parses");
    let b = Tour::from_json(camel).expect("camelCase parses");

    assert_eq!(a.id.as_str(), "t1");
    assert_eq!(a, b, "the two spellings are the same tour");

    // And the arithmetic works on it, rather than quietly producing nothing.
    let balances = tc_core::calculate(&b, tc_core::Options::default());
    assert_eq!(balances.per_person.len(), 2);
    assert_eq!(
        balances.get(&tc_core::PersonId::new("a")).unwrap().spent.0,
        1000
    );
}

/// Nothing is written that the source did not have - in particular, no nulls.
///
/// The mirror of `no_field_is_lost`, and the one that matters more. The C# model fills an
/// absent field with a default, and `Person.GroupId`'s default is a freshly generated Guid.
/// Writing `"GroupId": null` back does not leave the tour unchanged: it replaces a
/// generated value with nothing, and the settlement then dies on `GroupId.StartsWith(...)`
/// with a NullReferenceException - which is exactly how this was found, by feeding a
/// response from the Rust server to the C# calculator.
#[test]
fn no_null_is_invented() {
    for (name, json) in tour_files() {
        let tour = Tour::from_json(&json).unwrap();
        let before: Value = serde_json::from_str(&json).unwrap();
        let after: Value = serde_json::from_str(&tour.to_json().unwrap()).unwrap();

        let mut invented = Vec::new();
        find_invented_nulls(&before, &after, String::new(), &mut invented);

        assert!(
            invented.is_empty(),
            "[{name}] {} null(s) written where the source had no such field:\n  {}",
            invented.len(),
            invented.join("\n  ")
        );
    }
}

fn find_invented_nulls(before: &Value, after: &Value, path: String, out: &mut Vec<String>) {
    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, vb) in b {
                match a.get(k) {
                    Some(va) => find_invented_nulls(va, vb, format!("{path}.{k}"), out),
                    None if vb.is_null() => out.push(format!("{path}.{k}")),
                    None => {}
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (i, (va, vb)) in a.iter().zip(b).enumerate() {
                find_invented_nulls(va, vb, format!("{path}[{i}]"), out);
            }
        }
        _ => {}
    }
}
