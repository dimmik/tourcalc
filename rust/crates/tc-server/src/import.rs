//! `tc-server import <file.json>...` - tours from files into the configured store, once.
//!
//! A file is shaped like the InMemory seed file: a JSON array of tours, versions among them
//! (a version is a tour with `IsVersion` and `VersionFor_Id`, and the store files it under its
//! tour). `tools/store/fetch-tours.py` writes such a file from share links.
//!
//! Only adds: a tour or a version whose id the store already has is left as it is, so running
//! the same import twice changes nothing. Blank entries are skipped and of two entries with one
//! id the first wins - both as the InMemory store reads the seed file.
//!
//! Run it with the server stopped: a store that writes behind keeps a cache of its own, and two
//! processes on one cache and one branch would write over each other.

use crate::fields;
use crate::store::TourStore;
use std::collections::HashSet;
use tc_core::{Tour, TourId};

#[derive(Debug, Default, PartialEq)]
pub struct Imported {
    pub tours: usize,
    pub versions: usize,
    /// Already in the store, or a second entry with the same id.
    pub skipped: usize,
}

/// Reads the files first and stores nothing if one of them does not read.
pub async fn files(store: &dyn TourStore, paths: &[String]) -> Result<Imported, String> {
    let mut all = Vec::new();
    for p in paths {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        all.extend(tours_of(&text).map_err(|e| format!("{p}: {e}"))?);
    }
    Ok(add(store, all).await)
}

pub fn tours_of(text: &str) -> Result<Vec<Tour>, String> {
    let values: Vec<serde_json::Value> =
        serde_json::from_str(text).map_err(|e| format!("not a JSON array of tours: {e}"))?;
    Ok(values
        .into_iter()
        .filter_map(|v| Tour::from_json(&v.to_string()).ok())
        .filter(|t| !t.id.as_str().is_empty())
        .collect())
}

pub async fn add(store: &dyn TourStore, tours: Vec<Tour>) -> Imported {
    let mut done = Imported::default();
    let mut seen: HashSet<String> = HashSet::new();
    for tour in tours {
        let id = tour.id.as_str().to_owned();
        if !seen.insert(id.clone()) || store.get(&TourId::new(id)).await.is_some() {
            done.skipped += 1;
            continue;
        }
        if fields::is_version(&tour) {
            done.versions += 1;
        } else {
            done.tours += 1;
        }
        store.store(tour).await;
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryStore;

    const FILE: &str = r#"[
        {"Id": "a", "Name": "A", "AccessCodeMD5": "C1", "Persons": [], "Spendings": []},
        {"Id": "v1", "Name": "A", "AccessCodeMD5": "C1", "Persons": [], "Spendings": [],
         "IsVersion": true, "VersionFor_Id": "a", "VersionComment": "first"},
        {},
        {"Id": "a", "Name": "A again", "AccessCodeMD5": "C2", "Persons": [], "Spendings": []}
    ]"#;

    #[tokio::test]
    async fn tours_and_versions_go_in_once() {
        let store = InMemoryStore::empty();
        let first = add(&store, tours_of(FILE).unwrap()).await;
        assert_eq!(first, Imported { tours: 1, versions: 1, skipped: 1 });
        let a = store.get(&TourId::new("a")).await.unwrap();
        assert_eq!(fields::access_code(&a), "C1", "the first of two entries with one id");
        assert_eq!(store.versions(&TourId::new("a"), 0, 10).await.1, 1);

        let again = add(&store, tours_of(FILE).unwrap()).await;
        assert_eq!(again, Imported { tours: 0, versions: 0, skipped: 3 });
    }

    #[test]
    fn a_file_that_is_not_an_array_is_refused() {
        assert!(tours_of("{\"Id\": \"a\"}").is_err());
    }
}
