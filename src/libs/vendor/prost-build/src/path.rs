//! Utilities for working with Protobuf paths.

use std::collections::HashMap;
use std::iter;

/// Maps a fully-qualified Protobuf path to a value using path matchers.
#[derive(Debug, Default)]
pub(crate) struct PathMap<T> {
    matchers: HashMap<String, T>,
}

impl<T> PathMap<T> {
    /// Inserts a new matcher and associated value to the path map.
    pub(crate) fn insert(&mut self, matcher: String, value: T) {
        self.matchers.insert(matcher, value);
    }

    /// Returns the value which matches the provided fully-qualified Protobuf path.
    pub(crate) fn get(&self, fq_path: &'_ str) -> Option<&T> {
        // First, try matching the full path.
        iter::once(fq_path)
            // Then, try matching path suffixes.
            .chain(suffixes(fq_path))
            // Then, try matching path prefixes.
            .chain(prefixes(fq_path))
            // Then, match the global path. This matcher must never fail, since the constructor
            // initializes it.
            .chain(iter::once("."))
            .flat_map(|path| self.matchers.get(path))
            .next()
    }

    /// Returns the value which matches the provided fully-qualified Protobuf path and field name.
    pub(crate) fn get_field(&self, fq_path: &'_ str, field: &'_ str) -> Option<&T> {
        let full_path = format!("{}.{}", fq_path, field);
        let full_path = full_path.as_str();

        // First, try matching the path.
        let value = iter::once(full_path)
            // Then, try matching path suffixes.
            .chain(suffixes(full_path))
            // Then, try matching path suffixes without the field name.
            .chain(suffixes(fq_path))
            // Then, try matching path prefixes.
            .chain(prefixes(full_path))
            // Then, match the global path. This matcher must never fail, since the constructor
            // initializes it.
            .chain(iter::once("."))
            .flat_map(|path| self.matchers.get(path))
            .next();

        value
    }

    /// Removes all matchers from the path map.
    pub(crate) fn clear(&mut self) {
        self.matchers.clear();
    }
}

/// Given a fully-qualified path, returns a sequence of fully-qualified paths which match a prefix
/// of the input path, in decreasing path-length order.
///
/// Example: prefixes(".a.b.c.d") -> [".a.b.c", ".a.b", ".a"]
fn prefixes(fq_path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(fq_path), |path| {
        path.rsplitn(2, '.').nth(1).filter(|path| !path.is_empty())
    })
    .skip(1)
}

/// Given a fully-qualified path, returns a sequence of paths which match the suffix of the input
/// path, in decreasing path-length order.
///
/// Example: suffixes(".a.b.c.d") -> ["a.b.c.d", "b.c.d", "c.d", "d"]
fn suffixes(fq_path: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(fq_path), |path| {
        path.splitn(2, '.').nth(1).filter(|path| !path.is_empty())
    })
    .skip(1)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_prefixes() {
        assert_eq!(
            prefixes(".a.b.c.d").collect::<Vec<_>>(),
            vec![".a.b.c", ".a.b", ".a"],
        );
        assert_eq!(prefixes(".a").count(), 0);
        assert_eq!(prefixes(".").count(), 0);
    }

    #[test]
    fn test_suffixes() {
        assert_eq!(
            suffixes(".a.b.c.d").collect::<Vec<_>>(),
            vec!["a.b.c.d", "b.c.d", "c.d", "d"],
        );
        assert_eq!(suffixes(".a").collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(suffixes(".").collect::<Vec<_>>(), Vec::<&str>::new());
    }

    #[test]
    fn test_path_map_get() {
        let mut path_map = PathMap::default();
        path_map.insert(".a.b.c.d".to_owned(), 1);
        path_map.insert(".a.b".to_owned(), 2);
        path_map.insert("M1".to_owned(), 3);
        path_map.insert("M1.M2".to_owned(), 4);
        path_map.insert("M1.M2.f1".to_owned(), 5);
        path_map.insert("M1.M2.f2".to_owned(), 6);

        assert_eq!(None, path_map.get(".a.other"));
        assert_eq!(None, path_map.get(".a.bother"));
        assert_eq!(None, path_map.get(".other"));
        assert_eq!(None, path_map.get(".M1.other"));
        assert_eq!(None, path_map.get(".M1.M2.other"));

        assert_eq!(Some(&1), path_map.get(".a.b.c.d"));
        assert_eq!(Some(&1), path_map.get(".a.b.c.d.other"));

        assert_eq!(Some(&2), path_map.get(".a.b"));
        assert_eq!(Some(&2), path_map.get(".a.b.c"));
        assert_eq!(Some(&2), path_map.get(".a.b.other"));
        assert_eq!(Some(&2), path_map.get(".a.b.other.Other"));
        assert_eq!(Some(&2), path_map.get(".a.b.c.dother"));

        assert_eq!(Some(&3), path_map.get(".M1"));
        assert_eq!(Some(&3), path_map.get(".a.b.c.d.M1"));
        assert_eq!(Some(&3), path_map.get(".a.b.M1"));

        assert_eq!(Some(&4), path_map.get(".M1.M2"));
        assert_eq!(Some(&4), path_map.get(".a.b.c.d.M1.M2"));
        assert_eq!(Some(&4), path_map.get(".a.b.M1.M2"));

        assert_eq!(Some(&5), path_map.get(".M1.M2.f1"));
        assert_eq!(Some(&5), path_map.get(".a.M1.M2.f1"));
        assert_eq!(Some(&5), path_map.get(".a.b.M1.M2.f1"));

        assert_eq!(Some(&6), path_map.get(".M1.M2.f2"));
        assert_eq!(Some(&6), path_map.get(".a.M1.M2.f2"));
        assert_eq!(Some(&6), path_map.get(".a.b.M1.M2.f2"));

        // get_field

        assert_eq!(Some(&2), path_map.get_field(".a.b.Other", "other"));

        assert_eq!(Some(&4), path_map.get_field(".M1.M2", "other"));
        assert_eq!(Some(&4), path_map.get_field(".a.M1.M2", "other"));
        assert_eq!(Some(&4), path_map.get_field(".a.b.M1.M2", "other"));

        assert_eq!(Some(&5), path_map.get_field(".M1.M2", "f1"));
        assert_eq!(Some(&5), path_map.get_field(".a.M1.M2", "f1"));
        assert_eq!(Some(&5), path_map.get_field(".a.b.M1.M2", "f1"));

        assert_eq!(Some(&6), path_map.get_field(".M1.M2", "f2"));
        assert_eq!(Some(&6), path_map.get_field(".a.M1.M2", "f2"));
        assert_eq!(Some(&6), path_map.get_field(".a.b.M1.M2", "f2"));
    }
}
