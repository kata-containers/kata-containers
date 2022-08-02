// Copyright Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

mod toml_tree_ops {
    // The following pair of functions implement toml::Value tree merging, with
    // the second argument being merged into the first one and consumed in the
    // process.  The toml parser crate in use here doesn't support parsing into
    // a pre-existing (possibly pre-filled) TomlConfig instance but can parse
    // into a toml::Value tree so we use that instead.  All files (base and
    // drop-ins) are initially parsed into toml::Value trees which are
    // subsequently merged.  Only when the fully merged tree is computed it is
    // converted to a TomlConfig instance.

    fn merge_tables(base_table: &mut toml::value::Table, dropin_table: toml::value::Table) {
        for (key, val) in dropin_table.into_iter() {
            match base_table.get_mut(&key) {
                Some(base_val) => merge(base_val, val),
                None => {
                    base_table.insert(key, val);
                }
            }
        }
    }

    pub fn merge(base: &mut toml::Value, dropin: toml::Value) {
        match dropin {
            toml::Value::Table(dropin_table) => {
                if let toml::Value::Table(base_table) = base {
                    merge_tables(base_table, dropin_table);
                } else {
                    *base = toml::Value::Table(dropin_table);
                }
            }

            _ => *base = dropin,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        // Mock config structure to stand in for TomlConfig for low-level
        // toml::Value trees merging.
        #[derive(Deserialize, Debug, Default, PartialEq)]
        struct SubConfig {
            #[serde(default)]
            another_string: String,
            #[serde(default)]
            yet_another_number: i32,
            #[serde(default)]
            sub_array: Vec<i32>,
        }

        #[derive(Deserialize, Debug, Default, PartialEq)]
        struct Config {
            #[serde(default)]
            number: i32,
            #[serde(default)]
            string: String,
            #[serde(default)]
            another_number: u8,
            #[serde(default)]
            array: Vec<i32>,

            #[serde(default)]
            sub: SubConfig,
        }

        #[test]
        fn dropin_does_not_interfere_with_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                string = "foo"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    string: "foo".into(),
                    sub: Default::default(),
                    ..Default::default()
                })
            );
        }

        #[test]
        fn dropin_overrides_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                number = 43
                [sub]
                another_string = "bar"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 43,
                    sub: SubConfig {
                        another_string: "bar".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }

        #[test]
        fn dropin_extends_base() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                string = "hello"
                [sub]
                yet_another_number = 13
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    string: "hello".into(),
                    sub: SubConfig {
                        another_string: "foo".into(),
                        yet_another_number: 13,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }

        // Drop-ins can change the type of a value.  This might look weird but at
        // this level we have no idea about semantics so we just do what the
        // .toml's tell us.  The final type check is only performed by try_into().
        // Also, we don't necessarily test this because it's a desired feature.
        // It's just something that seems to follow from the way Value tree
        // merging is implemented so why not acknowledge and verify it.
        #[test]
        fn dropin_overrides_base_type() {
            let mut base: toml::Value = toml::from_str(
                r#"
                number = "foo"
                [sub]
                another_string = 42
            "#,
            )
            .unwrap();

            let dropin: toml::Value = toml::from_str(
                r#"
                number = 42
                [sub]
                another_string = "foo"
            "#,
            )
            .unwrap();

            merge(&mut base, dropin);

            assert_eq!(
                base.try_into(),
                Ok(Config {
                    number: 42,
                    sub: SubConfig {
                        another_string: "foo".into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
            );
        }
    }
}
