#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::derive_partial_eq_without_eq
)]

use indoc::indoc;
use serde_derive::Deserialize;
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::fmt::Debug;

fn test_de<T>(yaml: &str, expected: &T)
where
    T: serde::de::DeserializeOwned + PartialEq + Debug,
{
    let deserialized: T = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(*expected, deserialized);

    serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap();
    serde_yaml::from_str::<serde::de::IgnoredAny>(yaml).unwrap();
}

fn test_de_seed<T, S>(yaml: &str, seed: S, expected: &T)
where
    T: PartialEq + Debug,
    S: for<'de> serde::de::DeserializeSeed<'de, Value = T>,
{
    let deserialized: T = serde_yaml::seed::from_str_seed(yaml, seed).unwrap();
    assert_eq!(*expected, deserialized);

    serde_yaml::from_str::<serde_yaml::Value>(yaml).unwrap();
    serde_yaml::from_str::<serde::de::IgnoredAny>(yaml).unwrap();
}

#[test]
fn test_alias() {
    let yaml = indoc! {"
        ---
        first:
          &alias
          1
        second:
          *alias
        third: 3
    "};
    let mut expected = BTreeMap::new();
    {
        expected.insert(String::from("first"), 1);
        expected.insert(String::from("second"), 1);
        expected.insert(String::from("third"), 3);
    }
    test_de(yaml, &expected);
}

#[test]
fn test_option() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: Option<f64>,
        b: Option<String>,
        c: Option<bool>,
    }
    let yaml = indoc! {"
        ---
        b:
        c: true
    "};
    let expected = Data {
        a: None,
        b: None,
        c: Some(true),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_option_alias() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: Option<f64>,
        b: Option<String>,
        c: Option<bool>,
        d: Option<f64>,
        e: Option<String>,
        f: Option<bool>,
    }
    let yaml = indoc! {"
        ---
        none_f:
          &none_f
          ~
        none_s:
          &none_s
          ~
        none_b:
          &none_b
          ~

        some_f:
          &some_f
          1.0
        some_s:
          &some_s
          x
        some_b:
          &some_b
          true

        a: *none_f
        b: *none_s
        c: *none_b
        d: *some_f
        e: *some_s
        f: *some_b
    "};
    let expected = Data {
        a: None,
        b: None,
        c: None,
        d: Some(1.0),
        e: Some("x".to_owned()),
        f: Some(true),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_enum_alias() {
    #[derive(Deserialize, PartialEq, Debug)]
    enum E {
        A,
        B(u8, u8),
    }
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: E,
        b: E,
    }
    let yaml = indoc! {"
        ---
        aref:
          &aref
          A
        bref:
          &bref
          B:
            - 1
            - 2

        a: *aref
        b: *bref
    "};
    let expected = Data {
        a: E::A,
        b: E::B(1, 2),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_enum_tag() {
    #[derive(Deserialize, PartialEq, Debug)]
    enum E {
        A(String),
        B(String),
    }
    #[derive(Deserialize, PartialEq, Debug)]
    struct Data {
        a: E,
        b: E,
    }
    let yaml = indoc! {"
        ---
        a: !A foo
        b: !B bar
    "};
    let expected = Data {
        a: E::A("foo".into()),
        b: E::B("bar".into()),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_number_as_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Num {
        value: String,
    }
    let yaml = indoc! {"
        ---
        # Cannot be represented as u128
        value: 340282366920938463463374607431768211457
    "};
    let expected = Num {
        value: "340282366920938463463374607431768211457".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_i128_big() {
    let expected: i128 = ::std::i64::MIN as i128 - 1;
    let yaml = indoc! {"
        ---
        -9223372036854775809
    "};
    assert_eq!(expected, serde_yaml::from_str::<i128>(yaml).unwrap());
}

#[test]
fn test_u128_big() {
    let expected: u128 = ::std::u64::MAX as u128 + 1;
    let yaml = indoc! {"
        ---
        18446744073709551616
    "};
    assert_eq!(expected, serde_yaml::from_str::<u128>(yaml).unwrap());
}

#[test]
fn test_number_alias_as_string() {
    #[derive(Deserialize, PartialEq, Debug)]
    struct Num {
        version: String,
        value: String,
    }
    let yaml = indoc! {"
        ---
        version: &a 1.10
        value: *a
    "};
    let expected = Num {
        version: "1.10".to_owned(),
        value: "1.10".to_owned(),
    };
    test_de(yaml, &expected);
}

#[test]
fn test_de_mapping() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Data {
        pub substructure: serde_yaml::Mapping,
    }
    let yaml = indoc! {"
        ---
        substructure:
          a: 'foo'
          b: 'bar'
    "};

    let mut expected = Data {
        substructure: serde_yaml::Mapping::new(),
    };
    expected.substructure.insert(
        serde_yaml::Value::String("a".to_owned()),
        serde_yaml::Value::String("foo".to_owned()),
    );
    expected.substructure.insert(
        serde_yaml::Value::String("b".to_owned()),
        serde_yaml::Value::String("bar".to_owned()),
    );

    test_de(yaml, &expected);
}

#[test]
fn test_bomb() {
    #[derive(Debug, Deserialize, PartialEq)]
    struct Data {
        expected: String,
    }

    // This would deserialize an astronomical number of elements if we were
    // vulnerable.
    let yaml = indoc! {"
        ---
        a: &a ~
        b: &b [*a,*a,*a,*a,*a,*a,*a,*a,*a]
        c: &c [*b,*b,*b,*b,*b,*b,*b,*b,*b]
        d: &d [*c,*c,*c,*c,*c,*c,*c,*c,*c]
        e: &e [*d,*d,*d,*d,*d,*d,*d,*d,*d]
        f: &f [*e,*e,*e,*e,*e,*e,*e,*e,*e]
        g: &g [*f,*f,*f,*f,*f,*f,*f,*f,*f]
        h: &h [*g,*g,*g,*g,*g,*g,*g,*g,*g]
        i: &i [*h,*h,*h,*h,*h,*h,*h,*h,*h]
        j: &j [*i,*i,*i,*i,*i,*i,*i,*i,*i]
        k: &k [*j,*j,*j,*j,*j,*j,*j,*j,*j]
        l: &l [*k,*k,*k,*k,*k,*k,*k,*k,*k]
        m: &m [*l,*l,*l,*l,*l,*l,*l,*l,*l]
        n: &n [*m,*m,*m,*m,*m,*m,*m,*m,*m]
        o: &o [*n,*n,*n,*n,*n,*n,*n,*n,*n]
        p: &p [*o,*o,*o,*o,*o,*o,*o,*o,*o]
        q: &q [*p,*p,*p,*p,*p,*p,*p,*p,*p]
        r: &r [*q,*q,*q,*q,*q,*q,*q,*q,*q]
        s: &s [*r,*r,*r,*r,*r,*r,*r,*r,*r]
        t: &t [*s,*s,*s,*s,*s,*s,*s,*s,*s]
        u: &u [*t,*t,*t,*t,*t,*t,*t,*t,*t]
        v: &v [*u,*u,*u,*u,*u,*u,*u,*u,*u]
        w: &w [*v,*v,*v,*v,*v,*v,*v,*v,*v]
        x: &x [*w,*w,*w,*w,*w,*w,*w,*w,*w]
        y: &y [*x,*x,*x,*x,*x,*x,*x,*x,*x]
        z: &z [*y,*y,*y,*y,*y,*y,*y,*y,*y]
        expected: string
    "};

    let expected = Data {
        expected: "string".to_owned(),
    };

    assert_eq!(expected, serde_yaml::from_str::<Data>(yaml).unwrap());
}

#[test]
fn test_numbers() {
    let cases = [
        ("0xF0", "240"),
        ("+0xF0", "240"),
        ("-0xF0", "-240"),
        ("0o70", "56"),
        ("+0o70", "56"),
        ("-0o70", "-56"),
        ("0b10", "2"),
        ("+0b10", "2"),
        ("-0b10", "-2"),
        ("127", "127"),
        ("+127", "127"),
        ("-127", "-127"),
        (".inf", ".inf"),
        (".Inf", ".inf"),
        (".INF", ".inf"),
        ("-.inf", "-.inf"),
        ("-.Inf", "-.inf"),
        ("-.INF", "-.inf"),
        (".nan", ".nan"),
        (".NaN", ".nan"),
        (".NAN", ".nan"),
        ("0.1", "0.1"),
    ];
    for &(yaml, expected) in &cases {
        let value = serde_yaml::from_str::<Value>(yaml).unwrap();
        match value {
            Value::Number(number) => assert_eq!(number.to_string(), expected),
            _ => panic!("expected number. input={:?}, result={:?}", yaml, value),
        }
    }

    // NOT numbers.
    let cases = ["0127", "+0127", "-0127"];
    for yaml in &cases {
        let value = serde_yaml::from_str::<Value>(yaml).unwrap();
        match value {
            Value::String(string) => assert_eq!(string, *yaml),
            _ => panic!("expected string. input={:?}, result={:?}", yaml, value),
        }
    }
}

#[test]
fn test_stateful() {
    struct Seed(i64);

    impl<'de> serde::de::DeserializeSeed<'de> for Seed {
        type Value = i64;
        fn deserialize<D>(self, deserializer: D) -> Result<i64, D::Error>
        where
            D: serde::de::Deserializer<'de>,
        {
            struct Visitor(i64);
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = i64;

                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    write!(formatter, "an integer")
                }

                fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<i64, E> {
                    Ok(v * self.0)
                }

                fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<i64, E> {
                    Ok(v as i64 * self.0)
                }
            }

            deserializer.deserialize_any(Visitor(self.0))
        }
    }

    let cases = [("3", 5, 15), ("6", 7, 42), ("-5", 9, -45)];
    for &(yaml, seed, expected) in &cases {
        test_de_seed(yaml, Seed(seed), &expected);
    }
}
