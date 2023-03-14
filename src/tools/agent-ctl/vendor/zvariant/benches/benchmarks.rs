use byteorder::LE;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use zvariant::{
    from_slice_for_signature, to_bytes_for_signature, EncodingContext as Context, Type, Value,
};

fn fixed_size_array(c: &mut Criterion) {
    let ay = vec![77u8; 100_000];
    let ctxt = Context::<LE>::new_dbus(0);
    let signature = Vec::<u8>::signature();
    c.bench_function("byte_array_ser", |b| {
        b.iter(|| {
            to_bytes_for_signature(black_box(ctxt), black_box(&signature), black_box(&ay)).unwrap()
        })
    });
    let enc = to_bytes_for_signature(ctxt, &signature, &ay).unwrap();
    c.bench_function("byte_array_de", |b| {
        b.iter(|| {
            let _: Vec<u8> =
                from_slice_for_signature(black_box(&enc), black_box(ctxt), black_box(&signature))
                    .unwrap();
        })
    });
}

fn big_array_ser_and_de(c: &mut Criterion) {
    #[derive(Deserialize, Serialize, Type, PartialEq, Debug, Clone)]
    struct ZVField<'f> {
        int2: u64,
        string2: &'f str,
    }

    #[derive(Deserialize, Serialize, Type, PartialEq, Debug)]
    struct ZVStruct<'s> {
        string1: &'s str,
        int1: u64,
        field: ZVField<'s>,
        dict: HashMap<&'s str, Value<'s>>,
        int_array: Vec<u64>,
        string_array: Vec<&'s str>,
    }

    let mut dict = HashMap::new();
    let mut int_array = Vec::new();
    int_array.resize(1024 * 10, 0u64);
    let mut strings = Vec::new();
    let mut string_array: Vec<&str> = Vec::new();
    for idx in 0..1024 * 10 {
        strings.push(format!(
            "{}{}{}{}{}{}{}{}{}{}{}{}",
            idx, idx, idx, idx, idx, idx, idx, idx, idx, idx, idx, idx
        ));
    }
    for s in &strings {
        string_array.push(s.as_str());
        dict.insert(s.as_str(), Value::from(s.as_str()));
    }

    let element = ZVStruct {
        string1: "Testtest",
        int1: 0xFFFFFFFFFFFFFFFFu64,
        field: ZVField {
            string2: "TesttestTestest",
            int2: 0xFFFFFFFFFFFFFFFFu64,
        },
        int_array,
        string_array,
        dict,
    };

    // Let's try with DBus format first
    let ctxt = Context::<LE>::new_dbus(0);
    let signature = ZVStruct::signature();

    c.bench_function("big_array_ser_dbus", |b| {
        b.iter(|| {
            let encoded =
                to_bytes_for_signature(black_box(ctxt), black_box(&signature), black_box(&element))
                    .unwrap();
            black_box(encoded);
        })
    });

    let encoded = to_bytes_for_signature(ctxt, &signature, &element).unwrap();
    c.bench_function("big_array_de_dbus", |b| {
        b.iter(|| {
            let s: ZVStruct = from_slice_for_signature(
                black_box(&encoded),
                black_box(ctxt),
                black_box(&signature),
            )
            .unwrap();
            black_box(s);
        })
    });

    // Now GVariant.
    let ctxt = Context::<LE>::new_gvariant(0);

    c.bench_function("big_array_ser_gvariant", |b| {
        b.iter(|| {
            let encoded =
                to_bytes_for_signature(black_box(ctxt), black_box(&signature), black_box(&element))
                    .unwrap();
            black_box(encoded);
        })
    });

    let encoded = to_bytes_for_signature(ctxt, &signature, &element).unwrap();
    c.bench_function("big_array_de_gvariant", |b| {
        b.iter(|| {
            let s: ZVStruct = from_slice_for_signature(
                black_box(&encoded),
                black_box(ctxt),
                black_box(&signature),
            )
            .unwrap();
            black_box(s);
        })
    });
}

criterion_group!(benches, big_array_ser_and_de, fixed_size_array);
criterion_main!(benches);
