//! Who has asked to be told when a tour changes.
//!
//! A browser that grants permission hands the page an endpoint and two keys; the page sends
//! them here and the server keeps them against the tour. Nothing about a subscription
//! identifies a person - it is a URL at a push service and the keys to encrypt for it - and
//! it stops working the moment the browser withdraws it.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// What a browser hands over. The C#'s field names, because the client sends them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Subscription {
    #[serde(rename = "Url", alias = "url", alias = "endpoint")]
    pub url: String,
    #[serde(rename = "P256dh", alias = "p256dh", default)]
    pub p256dh: String,
    #[serde(rename = "Auth", alias = "auth", default)]
    pub auth: String,
}

impl Subscription {
    /// Two subscriptions are the same one if they point at the same endpoint - which is
    /// what the C# compares, and it is right: the keys can be renewed for the same browser.
    pub fn is_same(&self, other: &Subscription) -> bool {
        self.url == other.url
    }
}

pub trait SubscriptionStore: Send + Sync {
    fn add(&self, tour: &str, sub: Subscription);
    fn remove(&self, tour: &str, sub: &Subscription);
    fn has(&self, tour: &str, sub: &Subscription) -> bool;
    fn for_tour(&self, tour: &str) -> Vec<Subscription>;
}

#[derive(Default)]
pub struct InMemorySubscriptions {
    by_tour: RwLock<HashMap<String, Vec<Subscription>>>,
}

impl SubscriptionStore for InMemorySubscriptions {
    fn add(&self, tour: &str, sub: Subscription) {
        let mut all = self.by_tour.write().expect("subscriptions lock");
        let mine = all.entry(tour.to_owned()).or_default();
        // Subscribing twice from the same browser is one subscription, not two - otherwise
        // every reopened tab would add another copy of the same notification.
        if let Some(existing) = mine.iter_mut().find(|s| s.is_same(&sub)) {
            *existing = sub;
        } else {
            mine.push(sub);
        }
    }

    fn remove(&self, tour: &str, sub: &Subscription) {
        if let Some(mine) = self
            .by_tour
            .write()
            .expect("subscriptions lock")
            .get_mut(tour)
        {
            mine.retain(|s| !s.is_same(sub));
        }
    }

    fn has(&self, tour: &str, sub: &Subscription) -> bool {
        self.by_tour
            .read()
            .expect("subscriptions lock")
            .get(tour)
            .is_some_and(|mine| mine.iter().any(|s| s.is_same(sub)))
    }

    fn for_tour(&self, tour: &str) -> Vec<Subscription> {
        self.by_tour
            .read()
            .expect("subscriptions lock")
            .get(tour)
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(url: &str) -> Subscription {
        Subscription {
            url: url.to_owned(),
            p256dh: "key".into(),
            auth: "auth".into(),
        }
    }

    #[test]
    fn subscribing_twice_is_one_subscription() {
        let store = InMemorySubscriptions::default();
        store.add("t", sub("https://push.example/1"));
        store.add("t", sub("https://push.example/1"));
        assert_eq!(store.for_tour("t").len(), 1);
    }

    #[test]
    fn a_subscription_belongs_to_its_tour() {
        let store = InMemorySubscriptions::default();
        store.add("one", sub("https://push.example/1"));
        assert!(store.has("one", &sub("https://push.example/1")));
        assert!(!store.has("two", &sub("https://push.example/1")));
        assert!(store.for_tour("two").is_empty());
    }

    #[test]
    fn unsubscribing_takes_it_away() {
        let store = InMemorySubscriptions::default();
        store.add("t", sub("https://push.example/1"));
        store.add("t", sub("https://push.example/2"));
        store.remove("t", &sub("https://push.example/1"));
        assert_eq!(store.for_tour("t").len(), 1);
        assert!(store.has("t", &sub("https://push.example/2")));
    }

    /// The client sends what the C# model is called; older code sent the raw browser shape.
    #[test]
    fn either_spelling_is_read() {
        let csharp: Subscription =
            serde_json::from_str(r#"{"Url":"u","P256dh":"p","Auth":"a"}"#).unwrap();
        let browser: Subscription =
            serde_json::from_str(r#"{"endpoint":"u","p256dh":"p","auth":"a"}"#).unwrap();
        assert_eq!(csharp, browser);
    }
}
