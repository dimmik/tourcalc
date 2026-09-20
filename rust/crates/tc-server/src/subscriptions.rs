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

/// Where subscriptions are kept.
///
/// Asynchronous for the same reason [`crate::store::TourStore`] is: one of the two
/// implementations is a database on the other side of the network. The in-memory one does
/// not need it and pays nothing for it.
#[async_trait::async_trait]
pub trait SubscriptionStore: Send + Sync {
    async fn add(&self, tour: &str, sub: Subscription);
    async fn remove(&self, tour: &str, sub: &Subscription);
    async fn has(&self, tour: &str, sub: &Subscription) -> bool;
    async fn for_tour(&self, tour: &str) -> Vec<Subscription>;
    /// Every tour this browser is subscribed to, each once - the tour list's bells, asked
    /// in one question rather than one per tour.
    async fn tours_of(&self, sub: &Subscription) -> Vec<String>;
}

#[derive(Default)]
pub struct InMemorySubscriptions {
    by_tour: RwLock<HashMap<String, Vec<Subscription>>>,
}

#[async_trait::async_trait]
impl SubscriptionStore for InMemorySubscriptions {
    async fn add(&self, tour: &str, sub: Subscription) {
        let mut all = crate::lock::write(&self.by_tour);
        let mine = all.entry(tour.to_owned()).or_default();
        // Subscribing twice from the same browser is one subscription, not two - otherwise
        // every reopened tab would add another copy of the same notification.
        if let Some(existing) = mine.iter_mut().find(|s| s.is_same(&sub)) {
            *existing = sub;
        } else {
            mine.push(sub);
        }
    }

    async fn remove(&self, tour: &str, sub: &Subscription) {
        if let Some(mine) = crate::lock::write(&self.by_tour).get_mut(tour) {
            mine.retain(|s| !s.is_same(sub));
        }
    }

    async fn has(&self, tour: &str, sub: &Subscription) -> bool {
        crate::lock::read(&self.by_tour)
            .get(tour)
            .is_some_and(|mine| mine.iter().any(|s| s.is_same(sub)))
    }

    async fn for_tour(&self, tour: &str) -> Vec<Subscription> {
        crate::lock::read(&self.by_tour)
            .get(tour)
            .cloned()
            .unwrap_or_default()
    }

    async fn tours_of(&self, sub: &Subscription) -> Vec<String> {
        let mut tours: Vec<String> = crate::lock::read(&self.by_tour)
            .iter()
            .filter(|(_, mine)| mine.iter().any(|s| s.is_same(sub)))
            .map(|(tour, _)| tour.clone())
            .collect();
        tours.sort();
        tours
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

    #[tokio::test]
    async fn subscribing_twice_is_one_subscription() {
        let store = InMemorySubscriptions::default();
        store.add("t", sub("https://push.example/1")).await;
        store.add("t", sub("https://push.example/1")).await;
        assert_eq!(store.for_tour("t").await.len(), 1);
    }

    #[tokio::test]
    async fn a_subscription_belongs_to_its_tour() {
        let store = InMemorySubscriptions::default();
        store.add("one", sub("https://push.example/1")).await;
        assert!(store.has("one", &sub("https://push.example/1")).await);
        assert!(!store.has("two", &sub("https://push.example/1")).await);
        assert!(store.for_tour("two").await.is_empty());
    }

    #[tokio::test]
    async fn unsubscribing_takes_it_away() {
        let store = InMemorySubscriptions::default();
        store.add("t", sub("https://push.example/1")).await;
        store.add("t", sub("https://push.example/2")).await;
        store.remove("t", &sub("https://push.example/1")).await;
        assert_eq!(store.for_tour("t").await.len(), 1);
        assert!(store.has("t", &sub("https://push.example/2")).await);
    }

    #[tokio::test]
    async fn a_browser_knows_every_tour_it_is_subscribed_to() {
        let store = InMemorySubscriptions::default();
        store.add("one", sub("https://push.example/1")).await;
        store.add("two", sub("https://push.example/1")).await;
        store.add("two", sub("https://push.example/2")).await;
        store.add("three", sub("https://push.example/2")).await;
        assert_eq!(
            store.tours_of(&sub("https://push.example/1")).await,
            ["one", "two"]
        );
        assert!(store
            .tours_of(&sub("https://push.example/3"))
            .await
            .is_empty());
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
