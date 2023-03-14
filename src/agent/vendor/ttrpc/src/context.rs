// Copyright (c) 2021 Ant group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::proto::KeyValue;
use std::collections::HashMap;

#[derive(Clone, Default, Debug)]
pub struct Context {
    pub metadata: HashMap<String, Vec<String>>,
    pub timeout_nano: i64,
}

pub fn with_timeout(i: i64) -> Context {
    Context {
        timeout_nano: i,
        ..Default::default()
    }
}

pub fn with_metadata(md: HashMap<String, Vec<String>>) -> Context {
    Context {
        metadata: md,
        ..Default::default()
    }
}

impl Context {
    // appends additional values to the given key.
    pub fn add(&mut self, key: String, value: String) {
        if let Some(ref mut vl) = self.metadata.get_mut(&key) {
            vl.push(value);
        } else {
            self.metadata.insert(key.to_lowercase(), vec![value]);
        }
    }

    // Set sets the provided values for a given key.
    // The values will overwrite any existing values.
    // If no values provided, a key will be deleted.
    pub fn set(&mut self, key: String, value: Vec<String>) {
        if value.is_empty() {
            self.metadata.remove(&key);
        } else {
            self.metadata.insert(key.to_lowercase(), value);
        }
    }
}

pub fn from_pb(kvs: &protobuf::RepeatedField<KeyValue>) -> HashMap<String, Vec<String>> {
    let mut meta: HashMap<String, Vec<String>> = HashMap::new();
    for kv in kvs {
        if let Some(ref mut vl) = meta.get_mut(&kv.key) {
            vl.push(kv.value.clone());
        } else {
            meta.insert(kv.key.clone(), vec![kv.value.clone()]);
        }
    }
    meta
}

pub fn to_pb(kvs: HashMap<String, Vec<String>>) -> protobuf::RepeatedField<KeyValue> {
    let mut meta: protobuf::RepeatedField<KeyValue> = protobuf::RepeatedField::default();
    for (k, vl) in kvs {
        for v in vl {
            let key = KeyValue {
                key: k.clone(),
                value: v.clone(),
                ..Default::default()
            };
            meta.push(key);
        }
    }
    meta
}

#[cfg(test)]
mod tests {
    use crate::context;
    use crate::proto::KeyValue;

    #[test]
    fn test_metadata() {
        // RepeatedField -> HashMap, test from_pb()
        let mut src: protobuf::RepeatedField<KeyValue> = protobuf::RepeatedField::default();
        for i in &[
            ("key1", "value1-1"),
            ("key1", "value1-2"),
            ("key2", "value2"),
        ] {
            let key = KeyValue {
                key: i.0.to_string(),
                value: i.1.to_string(),
                ..Default::default()
            };
            src.push(key);
        }

        let dst = context::from_pb(&src);
        assert_eq!(dst.len(), 2);

        assert_eq!(
            dst.get("key1"),
            Some(&vec!["value1-1".to_string(), "value1-2".to_string()])
        );
        assert_eq!(dst.get("key2"), Some(&vec!["value2".to_string()]));
        assert_eq!(dst.get("key3"), None);

        // HashMap -> RepeatedField , test to_pb()
        let src = context::to_pb(dst);
        let mut kvs = src.into_vec();
        kvs.sort_by(|a, b| a.key.partial_cmp(&b.key).unwrap());

        assert_eq!(kvs.len(), 3);

        assert_eq!(kvs[0].key, "key1");
        assert_eq!(kvs[0].value, "value1-1");

        assert_eq!(kvs[1].key, "key1");
        assert_eq!(kvs[1].value, "value1-2");

        assert_eq!(kvs[2].key, "key2");
        assert_eq!(kvs[2].value, "value2");
    }

    #[test]
    fn test_context() {
        let ctx: context::Context = Default::default();
        assert_eq!(0, ctx.timeout_nano);
        assert_eq!(ctx.metadata.len(), 0);

        let mut ctx = context::with_timeout(99);
        assert_eq!(99, ctx.timeout_nano);
        assert_eq!(ctx.metadata.len(), 0);

        ctx.add("key1".to_string(), "value1-1".to_string());
        assert_eq!(ctx.metadata.len(), 1);
        assert_eq!(
            ctx.metadata.get("key1"),
            Some(&vec!["value1-1".to_string()])
        );

        ctx.add("key1".to_string(), "value1-2".to_string());
        assert_eq!(ctx.metadata.len(), 1);
        assert_eq!(
            ctx.metadata.get("key1"),
            Some(&vec!["value1-1".to_string(), "value1-2".to_string()])
        );

        ctx.set("key2".to_string(), vec!["value2".to_string()]);
        assert_eq!(ctx.metadata.len(), 2);
        assert_eq!(ctx.metadata.get("key2"), Some(&vec!["value2".to_string()]));

        ctx.set("key1".to_string(), vec![]);
        assert_eq!(ctx.metadata.len(), 1);
        assert_eq!(ctx.metadata.get("key1"), None);
    }
}
