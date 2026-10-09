// SPDX-License-Identifier: GPL-3.0-or-later
//! The autoupdate stream's data: `collection/id/field` → JSON value. Each message replaces the
//! listed keys; `null` removes a key.

use std::collections::HashMap;

use serde_json::{Map, Value};

#[derive(Debug, Default, Clone)]
pub struct Store {
    values: HashMap<String, Value>,
}

impl Store {
    pub fn new() -> Self {
        Store::default()
    }

    /// Applies one autoupdate message. Returns whether anything changed.
    pub fn apply(&mut self, update: Map<String, Value>) -> bool {
        let mut changed = false;
        for (key, value) in update {
            if value.is_null() {
                changed |= self.values.remove(&key).is_some();
            } else if self.values.get(&key) != Some(&value) {
                self.values.insert(key, value);
                changed = true;
            }
        }
        changed
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn get(&self, collection: &str, id: u32, field: &str) -> Option<&Value> {
        self.values.get(&format!("{collection}/{id}/{field}"))
    }

    pub fn str(&self, collection: &str, id: u32, field: &str) -> String {
        match self.get(collection, id, field) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => String::new(),
        }
    }

    pub fn u32(&self, collection: &str, id: u32, field: &str) -> Option<u32> {
        self.get(collection, id, field)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    }

    pub fn i64(&self, collection: &str, id: u32, field: &str) -> Option<i64> {
        self.get(collection, id, field).and_then(Value::as_i64)
    }

    pub fn bool(&self, collection: &str, id: u32, field: &str) -> bool {
        self.get(collection, id, field)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    pub fn ids(&self, collection: &str, id: u32, field: &str) -> Vec<u32> {
        match self.get(collection, id, field) {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|n| u32::try_from(n).ok())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// A generic relation (`motion/3`) as `(collection, id)`.
    pub fn fqid(&self, collection: &str, id: u32, field: &str) -> Option<(String, u32)> {
        let s = self.get(collection, id, field)?.as_str()?;
        parse_fqid(s)
    }

    /// Whether the object exists (has an `id` field, as every request asks for it).
    pub fn exists(&self, collection: &str, id: u32) -> bool {
        self.get(collection, id, "id").is_some()
    }
}

pub fn parse_fqid(s: &str) -> Option<(String, u32)> {
    let (c, id) = s.split_once('/')?;
    Some((c.to_owned(), id.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn updates_replace_and_null_removes() {
        let mut s = Store::new();
        assert!(s.apply(map(
            json!({"motion/1/id": 1, "motion/1/title": "A", "motion/1/submitter_ids": [2, 3]})
        )));
        assert_eq!(s.str("motion", 1, "title"), "A");
        assert_eq!(s.ids("motion", 1, "submitter_ids"), vec![2, 3]);
        assert!(
            !s.apply(map(json!({"motion/1/title": "A"}))),
            "same value is no change"
        );
        assert!(s.apply(map(json!({"motion/1/title": null}))));
        assert_eq!(s.str("motion", 1, "title"), "");
        assert!(s.exists("motion", 1));
        assert!(!s.exists("motion", 2));
        s.apply(map(json!({"projection/4/content_object_id": "motion/1"})));
        assert_eq!(
            s.fqid("projection", 4, "content_object_id"),
            Some(("motion".into(), 1))
        );
        assert_eq!(parse_fqid("meeting/x"), None);
    }
}
