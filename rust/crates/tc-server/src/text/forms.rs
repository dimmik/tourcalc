//! Reading what a form posted.
//!
//! `application/x-www-form-urlencoded` by hand, for one reason: a form here can send the
//! same name more than once - the "for these people" checkboxes are a list - and the
//! deserialiser axum's `Form` uses maps each name to a single value and quietly keeps the
//! last. A list of people silently becoming one person is exactly the kind of bug that
//! looks like an arithmetic error three screens later.

use std::collections::HashMap;

/// The fields of a posted form, each with everything sent under that name.
pub struct Fields(HashMap<String, Vec<String>>);

impl Fields {
    pub fn parse(body: &str) -> Fields {
        let mut fields: HashMap<String, Vec<String>> = HashMap::new();
        for (key, value) in form_urlencoded::parse(body.as_bytes()) {
            fields
                .entry(key.into_owned())
                .or_default()
                .push(value.into_owned());
        }
        Fields(fields)
    }

    /// The first value sent under this name, trimmed; empty if it was not sent at all.
    pub fn text(&self, name: &str) -> &str {
        self.0
            .get(name)
            .and_then(|v| v.first())
            .map(|s| s.trim())
            .unwrap_or("")
    }

    /// Everything sent under this name - a list of checkboxes, say.
    pub fn all(&self, name: &str) -> &[String] {
        self.0.get(name).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// A checkbox: browsers send nothing at all when it is not ticked.
    pub fn checked(&self, name: &str) -> bool {
        !self.text(name).is_empty()
    }

    pub fn number(&self, name: &str) -> Option<i64> {
        // A person may well type "1 200" or "1200,50"; take what was meant.
        let raw: String = self
            .text(name)
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '-')
            .collect();
        raw.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_name_keeps_every_value() {
        let f = Fields::parse("ToGuid=a&ToGuid=b&ToGuid=c&Name=x");
        assert_eq!(f.all("ToGuid"), ["a", "b", "c"]);
        assert_eq!(f.text("Name"), "x");
    }

    #[test]
    fn spaces_and_letters_survive_the_encoding() {
        let f = Fields::parse("Name=%D0%92%D0%B0%D1%81%D1%8F+%D0%9F&Description=a%26b");
        assert_eq!(f.text("Name"), "Вася П");
        assert_eq!(f.text("Description"), "a&b");
    }

    #[test]
    fn an_unticked_checkbox_sends_nothing() {
        let f = Fields::parse("ToAll=true");
        assert!(f.checked("ToAll"));
        assert!(!f.checked("IsDryRun"));
    }

    #[test]
    fn an_amount_can_be_typed_the_way_people_type_it() {
        assert_eq!(Fields::parse("Amount=1+200").number("Amount"), Some(1200));
        assert_eq!(Fields::parse("Amount=-50").number("Amount"), Some(-50));
        assert_eq!(Fields::parse("Amount=").number("Amount"), None);
    }
}
