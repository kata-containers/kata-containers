//! Let it torture the implementation with some randomized operations.

extern crate arc_swap;
extern crate crossbeam_utils;
extern crate once_cell;
extern crate proptest;

use std::mem;
use std::sync::Arc;

use arc_swap::ArcSwap;
use once_cell::sync::Lazy;
use proptest::prelude::*;

#[derive(Copy, Clone, Debug)]
enum OpsInstruction {
    Store(usize),
    Swap(usize),
    LoadFull,
    LoadSignalSafe,
    Load,
}

impl OpsInstruction {
    fn random() -> impl Strategy<Value = Self> {
        prop_oneof![
            any::<usize>().prop_map(Self::Store),
            any::<usize>().prop_map(Self::Swap),
            Just(Self::LoadFull),
            Just(Self::LoadSignalSafe),
            Just(Self::Load),
        ]
    }
}

proptest! {
    #[test]
    fn ops(instructions in proptest::collection::vec(OpsInstruction::random(), 1..100)) {
        use OpsInstruction::*;
        let mut m = 0;
        let a = ArcSwap::from_pointee(0usize);
        for ins in instructions {
            match ins {
                Store(v) => {
                    m = v;
                    a.store(Arc::new(v));
                }
                Swap(v) => {
                    let old = mem::replace(&mut m, v);
                    assert_eq!(old, *a.swap(Arc::new(v)));
                }
                Load => assert_eq!(m, **a.load()),
                LoadFull => assert_eq!(m, *a.load_full()),
                LoadSignalSafe => assert_eq!(m, **a.load_signal_safe()),
            }
        }
    }
    /*
    use model::Shared;

    linearizable! {
        Implementation => let a = Shared::new(ArcSwap::from(Arc::clone(&ARCS[0]))),
        Store(usize)(idx in 0..LIMIT) -> () {
            a.store(Arc::clone(&ARCS[idx]));
        },
        Load(())(() in any::<()>()) -> usize {
            **a.load()
        },
        Cas((usize, usize))((current, new) in (0..LIMIT, 0..LIMIT)) -> usize {
            let new = Arc::clone(&ARCS[new]);
            **a.compare_and_swap(&ARCS[current], new)
        }
    }
}
    */
}

const LIMIT: usize = 5;
static ARCS: Lazy<Vec<Arc<usize>>> = Lazy::new(|| (0..LIMIT).map(Arc::new).collect());

#[derive(Copy, Clone, Debug)]
enum SelInstruction {
    Swap(usize),
    Cas(usize, usize),
}

impl SelInstruction {
    fn random() -> impl Strategy<Value = Self> {
        prop_oneof![
            (0..LIMIT).prop_map(Self::Swap),
            (0..LIMIT, 0..LIMIT).prop_map(|(cur, new)| Self::Cas(cur, new)),
        ]
    }
}

proptest! {
    #[test]
    fn selection(instructions in proptest::collection::vec(SelInstruction::random(), 1..100)) {
        let mut bare = Arc::clone(&ARCS[0]);
        let a = ArcSwap::from(Arc::clone(&ARCS[0]));
        for ins in instructions {
            match ins {
                SelInstruction::Swap(idx) => {
                    let expected = mem::replace(&mut bare, Arc::clone(&ARCS[idx]));
                    let actual = a.swap(Arc::clone(&ARCS[idx]));
                    assert!(Arc::ptr_eq(&expected, &actual));
                }
                SelInstruction::Cas(cur, new) => {
                    let expected = Arc::clone(&bare);
                    if bare == ARCS[cur] {
                        bare = Arc::clone(&ARCS[new]);
                    }
                    let actual = a.compare_and_swap(&ARCS[cur], Arc::clone(&ARCS[new]));
                    assert!(Arc::ptr_eq(&expected, &actual));
                }
            }
        }
    }
}
