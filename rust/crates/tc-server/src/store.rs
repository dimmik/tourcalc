//! Where the tours live.
//!
//! **This is the file to read for shared ownership.** The C# in-memory store answers every
//! read with a full clone of the tour - serialised to JSON and parsed back - because
//! otherwise a caller could reach into the store's own copy and change it:
//!
//! ```csharp
//! public Tour GetTour(string tourid) {
//!     var tour = Tours.FirstOrDefault(t => t.Id == tourid);
//!     // return a clone
//!     return JsonConvert.DeserializeObject<Tour>(JsonConvert.SerializeObject(tour));
//! }
//! ```
//!
//! Here a read hands out an `Arc<Tour>`: the tour itself is not copied, only a count of how
//! many people are looking at it. Nobody can change it through that handle, because `Arc`
//! gives out shared references and shared references are read-only - the compiler, not a
//! convention, is what stops it.
//!
//! Writing is where it gets interesting. `Arc::make_mut` clones the tour **only if somebody
//! else is still holding it**, and otherwise edits it where it lies. One copy, made exactly
//! when a copy is unavoidable, instead of one per read.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tc_core::{Tour, TourId};

/// Anything that can answer for the tours. A trait because the real one will be MongoDB,
/// and because the tests should not need it.
pub trait TourStore: Send + Sync {
    fn get(&self, id: &TourId) -> Option<Arc<Tour>>;
    /// Every tour the given access codes may see, newest first.
    fn list(&self, allowed: &dyn Fn(&Tour) -> bool) -> Vec<Arc<Tour>>;
    /// Writes a tour, adding it if its id is new.
    fn store(&self, tour: Tour);
    /// Removes a tour; `false` if there was none.
    fn remove(&self, id: &TourId) -> bool;
}

/// Tours held in memory, seeded from a file at startup and never written back.
///
/// `Send + Sync` is what lets axum share one of these across every connection at once, and
/// it is not something to declare - it follows from what the type is made of. `RwLock` and
/// `Arc` are both, so this is too, and the compiler will say so if that ever stops holding.
pub struct InMemoryStore {
    tours: RwLock<HashMap<TourId, Arc<Tour>>>,
    /// Insertion order, so a list can come back the way the file had it.
    order: RwLock<Vec<TourId>>,
}

impl InMemoryStore {
    pub fn empty() -> Self {
        InMemoryStore {
            tours: RwLock::new(HashMap::new()),
            order: RwLock::new(Vec::new()),
        }
    }

    /// Reads the same seed file the C# server reads.
    pub fn from_seed_file(path: &std::path::Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_seed_json(&text)
    }

    pub fn from_seed_json(text: &str) -> Result<Self, String> {
        // The file is an array of tours, and some entries are blank - the C# skips those by
        // ending up with an empty id, so this does the same rather than failing the boot.
        let values: Vec<serde_json::Value> =
            serde_json::from_str(text).map_err(|e| format!("seed file: {e}"))?;

        let store = InMemoryStore::empty();
        for v in values {
            let Ok(tour) = Tour::from_json(&v.to_string()) else {
                continue;
            };
            if tour.id.as_str().is_empty() {
                continue;
            }
            store.put(tour);
        }
        Ok(store)
    }

    /// Adds a tour, keeping the first of any duplicate ids.
    ///
    /// The seed file contains the same tour id twice under two different access codes. The
    /// C# store keeps a plain list and answers with `FirstOrDefault`, so the first one wins
    /// there; letting the later one overwrite made the tour invisible to the code that
    /// owns it, and the tour list came back two entries short.
    pub fn put(&self, tour: Tour) {
        let id = tour.id.clone();
        let mut tours = self.tours.write().expect("store lock");
        if !tours.contains_key(&id) {
            tours.insert(id.clone(), Arc::new(tour));
            self.order.write().expect("store lock").push(id);
        }
    }

    /// Changes a tour in place, cloning it only if somebody else is reading it right now.
    ///
    /// Unused while the server is read-only; here because it is the other half of the point
    /// this module makes, and it is three lines.
    pub fn update(&self, id: &TourId, f: impl FnOnce(&mut Tour)) -> bool {
        let mut tours = self.tours.write().expect("store lock");
        match tours.get_mut(id) {
            Some(arc) => {
                f(Arc::make_mut(arc));
                true
            }
            None => false,
        }
    }

    pub fn len(&self) -> usize {
        self.tours.read().expect("store lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TourStore for InMemoryStore {
    fn store(&self, tour: Tour) {
        let id = tour.id.clone();
        let mut tours = self.tours.write().expect("store lock");
        if tours.insert(id.clone(), Arc::new(tour)).is_none() {
            self.order.write().expect("store lock").push(id);
        }
    }

    fn remove(&self, id: &TourId) -> bool {
        let mut tours = self.tours.write().expect("store lock");
        if tours.remove(id).is_some() {
            self.order.write().expect("store lock").retain(|i| i != id);
            true
        } else {
            false
        }
    }

    fn get(&self, id: &TourId) -> Option<Arc<Tour>> {
        // `.cloned()` on an Option<&Arc<Tour>> clones the Arc - a counter bump - and not
        // the tour behind it. This is the line the C# spends a JSON round trip on.
        self.tours.read().expect("store lock").get(id).cloned()
    }

    fn list(&self, allowed: &dyn Fn(&Tour) -> bool) -> Vec<Arc<Tour>> {
        let tours = self.tours.read().expect("store lock");
        self.order
            .read()
            .expect("store lock")
            .iter()
            .filter_map(|id| tours.get(id))
            .filter(|t| allowed(t))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed() -> InMemoryStore {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/zscph2y.tour.json");
        let one = std::fs::read_to_string(path).unwrap();
        InMemoryStore::from_seed_json(&format!("[{one}]")).unwrap()
    }

    #[test]
    fn reads_a_tour_without_copying_it() {
        let store = seed();
        let id = TourId::new("zscph2y");
        let a = store.get(&id).unwrap();
        let b = store.get(&id).unwrap();
        // Two handles onto the same tour, not two tours.
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.persons.len(), 10);
    }

    #[test]
    fn writing_while_somebody_reads_leaves_the_reader_alone() {
        let store = seed();
        let id = TourId::new("zscph2y");

        let held = store.get(&id).unwrap();
        let name_before = held.name.clone();

        store.update(&id, |t| t.name = "renamed".to_owned());

        // The reader still sees what it was given: make_mut copied rather than reaching
        // into a tour somebody was holding.
        assert_eq!(held.name, name_before);
        assert_eq!(store.get(&id).unwrap().name, "renamed");
    }

    #[test]
    fn a_missing_tour_is_none() {
        assert!(seed().get(&TourId::new("nope")).is_none());
    }
}
