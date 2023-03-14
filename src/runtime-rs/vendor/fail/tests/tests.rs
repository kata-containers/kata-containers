// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::*;
use std::time::*;
use std::*;

use fail::fail_point;

#[test]
fn test_off() {
    let f = || {
        fail_point!("off", |_| 2);
        0
    };
    assert_eq!(f(), 0);

    fail::cfg("off", "off").unwrap();
    assert_eq!(f(), 0);
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_return() {
    let f = || {
        fail_point!("return", |s: Option<String>| s
            .map_or(2, |s| s.parse().unwrap()));
        0
    };
    assert_eq!(f(), 0);

    fail::cfg("return", "return(1000)").unwrap();
    assert_eq!(f(), 1000);

    fail::cfg("return", "return").unwrap();
    assert_eq!(f(), 2);
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_sleep() {
    let f = || {
        fail_point!("sleep");
    };
    let timer = Instant::now();
    f();
    assert!(timer.elapsed() < Duration::from_millis(1000));

    let timer = Instant::now();
    fail::cfg("sleep", "sleep(1000)").unwrap();
    f();
    assert!(timer.elapsed() > Duration::from_millis(1000));
}

#[test]
#[should_panic]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_panic() {
    let f = || {
        fail_point!("panic");
    };
    fail::cfg("panic", "panic(msg)").unwrap();
    f();
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_print() {
    struct LogCollector(Arc<Mutex<Vec<String>>>);
    impl log::Log for LogCollector {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            let mut buf = self.0.lock().unwrap();
            buf.push(format!("{}", record.args()));
        }
        fn flush(&self) {}
    }

    let buffer = Arc::new(Mutex::new(vec![]));
    let collector = LogCollector(buffer.clone());
    log::set_max_level(log::LevelFilter::Info);
    log::set_boxed_logger(Box::new(collector)).unwrap();

    let f = || {
        fail_point!("print");
    };
    fail::cfg("print", "print(msg)").unwrap();
    f();
    let msg = buffer.lock().unwrap().pop().unwrap();
    assert_eq!(msg, "msg");

    fail::cfg("print", "print").unwrap();
    f();
    let msg = buffer.lock().unwrap().pop().unwrap();
    assert_eq!(msg, "failpoint print executed.");
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_pause() {
    let f = || {
        fail_point!("pause");
    };
    f();

    fail::cfg("pause", "pause").unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        // pause
        tx.send(f()).unwrap();
        // woken up by new order pause, and then pause again.
        tx.send(f()).unwrap();
        // woken up by remove, and then quit immediately.
        tx.send(f()).unwrap();
    });

    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    fail::cfg("pause", "pause").unwrap();
    rx.recv_timeout(Duration::from_millis(500)).unwrap();

    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    fail::remove("pause");
    rx.recv_timeout(Duration::from_millis(500)).unwrap();

    rx.recv_timeout(Duration::from_millis(500)).unwrap();
}

#[test]
fn test_yield() {
    let f = || {
        fail_point!("yield");
    };
    fail::cfg("test", "yield").unwrap();
    f();
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_callback() {
    let f1 = || {
        fail_point!("cb");
    };
    let f2 = || {
        fail_point!("cb");
    };

    let counter = Arc::new(AtomicUsize::new(0));
    let counter2 = counter.clone();
    fail::cfg_callback("cb", move || {
        counter2.fetch_add(1, Ordering::SeqCst);
    })
    .unwrap();
    f1();
    f2();
    assert_eq!(2, counter.load(Ordering::SeqCst));
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_delay() {
    let f = || fail_point!("delay");
    let timer = Instant::now();
    fail::cfg("delay", "delay(1000)").unwrap();
    f();
    assert!(timer.elapsed() > Duration::from_millis(1000));
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_freq_and_count() {
    let f = || {
        fail_point!("freq_and_count", |s: Option<String>| s
            .map_or(2, |s| s.parse().unwrap()));
        0
    };
    fail::cfg(
        "freq_and_count",
        "50%50*return(1)->50%50*return(-1)->50*return",
    )
    .unwrap();
    let mut sum = 0;
    for _ in 0..5000 {
        let res = f();
        sum += res;
    }
    assert_eq!(sum, 100);
}

#[test]
#[cfg_attr(not(feature = "failpoints"), ignore)]
fn test_condition() {
    let f = |_enabled| {
        fail_point!("condition", _enabled, |_| 2);
        0
    };
    assert_eq!(f(false), 0);

    fail::cfg("condition", "return").unwrap();
    assert_eq!(f(false), 0);

    assert_eq!(f(true), 2);
}

#[test]
fn test_list() {
    assert!(!fail::list().contains(&("list".to_string(), "off".to_string())));
    fail::cfg("list", "off").unwrap();
    assert!(fail::list().contains(&("list".to_string(), "off".to_string())));
    fail::cfg("list", "return").unwrap();
    assert!(fail::list().contains(&("list".to_string(), "return".to_string())));
}
