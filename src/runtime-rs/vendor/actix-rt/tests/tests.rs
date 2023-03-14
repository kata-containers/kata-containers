use std::{
    future::Future,
    time::{Duration, Instant},
};

use actix_rt::{task::JoinError, Arbiter, System};

#[cfg(not(feature = "io-uring"))]
use {
    std::{sync::mpsc::channel, thread},
    tokio::sync::oneshot,
};

#[test]
fn await_for_timer() {
    let time = Duration::from_secs(1);
    let instant = Instant::now();
    System::new().block_on(async move {
        tokio::time::sleep(time).await;
    });
    assert!(
        instant.elapsed() >= time,
        "Block on should poll awaited future to completion"
    );
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn run_with_code() {
    let sys = System::new();
    System::current().stop_with_code(42);
    let exit_code = sys.run_with_code().expect("system stop should not error");
    assert_eq!(exit_code, 42);
}

#[test]
fn join_another_arbiter() {
    let time = Duration::from_secs(1);
    let instant = Instant::now();
    System::new().block_on(async move {
        let arbiter = Arbiter::new();
        arbiter.spawn(Box::pin(async move {
            tokio::time::sleep(time).await;
            Arbiter::current().stop();
        }));
        arbiter.join().unwrap();
    });
    assert!(
        instant.elapsed() >= time,
        "Join on another arbiter should complete only when it calls stop"
    );

    let instant = Instant::now();
    System::new().block_on(async move {
        let arbiter = Arbiter::new();
        arbiter.spawn_fn(move || {
            actix_rt::spawn(async move {
                tokio::time::sleep(time).await;
                Arbiter::current().stop();
            });
        });
        arbiter.join().unwrap();
    });
    assert!(
        instant.elapsed() >= time,
        "Join on an arbiter that has used actix_rt::spawn should wait for said future"
    );

    let instant = Instant::now();
    System::new().block_on(async move {
        let arbiter = Arbiter::new();
        arbiter.spawn(Box::pin(async move {
            tokio::time::sleep(time).await;
            Arbiter::current().stop();
        }));
        arbiter.stop();
        arbiter.join().unwrap();
    });
    assert!(
        instant.elapsed() < time,
        "Premature stop of arbiter should conclude regardless of it's current state"
    );
}

#[test]
fn non_static_block_on() {
    let string = String::from("test_str");
    let string = string.as_str();

    let sys = System::new();

    sys.block_on(async {
        actix_rt::time::sleep(Duration::from_millis(1)).await;
        assert_eq!("test_str", string);
    });

    let rt = actix_rt::Runtime::new().unwrap();

    rt.block_on(async {
        actix_rt::time::sleep(Duration::from_millis(1)).await;
        assert_eq!("test_str", string);
    });
}

#[test]
fn wait_for_spawns() {
    let rt = actix_rt::Runtime::new().unwrap();

    let handle = rt.spawn(async {
        println!("running on the runtime");
        // panic is caught at task boundary
        panic!("intentional test panic");
    });

    assert!(rt.block_on(handle).is_err());
}

// Temporary disabled tests for io-uring feature.
// They should be enabled when possible.

#[cfg(not(feature = "io-uring"))]
#[test]
fn arbiter_spawn_fn_runs() {
    let _ = System::new();

    let (tx, rx) = channel::<u32>();

    let arbiter = Arbiter::new();
    arbiter.spawn_fn(move || tx.send(42).unwrap());

    let num = rx.recv().unwrap();
    assert_eq!(num, 42);

    arbiter.stop();
    arbiter.join().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn arbiter_handle_spawn_fn_runs() {
    let sys = System::new();

    let (tx, rx) = channel::<u32>();

    let arbiter = Arbiter::new();
    let handle = arbiter.handle();
    drop(arbiter);

    handle.spawn_fn(move || {
        tx.send(42).unwrap();
        System::current().stop()
    });

    let num = rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(num, 42);

    handle.stop();
    sys.run().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn arbiter_drop_no_panic_fn() {
    let _ = System::new();

    let arbiter = Arbiter::new();
    arbiter.spawn_fn(|| panic!("test"));

    arbiter.stop();
    arbiter.join().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn arbiter_drop_no_panic_fut() {
    let _ = System::new();

    let arbiter = Arbiter::new();
    arbiter.spawn(async { panic!("test") });

    arbiter.stop();
    arbiter.join().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn system_arbiter_spawn() {
    let runner = System::new();

    let (tx, rx) = oneshot::channel();
    let sys = System::current();

    thread::spawn(|| {
        // this thread will have no arbiter in it's thread local so call will panic
        Arbiter::current();
    })
    .join()
    .unwrap_err();

    let thread = thread::spawn(|| {
        // this thread will have no arbiter in it's thread local so use the system handle instead
        System::set_current(sys);
        let sys = System::current();

        let arb = sys.arbiter();
        arb.spawn(async move {
            tx.send(42u32).unwrap();
            System::current().stop();
        });
    });

    assert_eq!(runner.block_on(rx).unwrap(), 42);
    thread.join().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn system_stop_stops_arbiters() {
    let sys = System::new();
    let arb = Arbiter::new();

    // arbiter should be alive to receive spawn msg
    assert!(Arbiter::current().spawn_fn(|| {}));
    assert!(arb.spawn_fn(|| {}));

    System::current().stop();
    sys.run().unwrap();

    // account for slightly slow thread de-spawns
    thread::sleep(Duration::from_millis(500));

    // arbiter should be dead and return false
    assert!(!Arbiter::current().spawn_fn(|| {}));
    assert!(!arb.spawn_fn(|| {}));

    arb.join().unwrap();
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn new_system_with_tokio() {
    let (tx, rx) = channel();

    let res = System::with_tokio_rt(move || {
        tokio::runtime::Builder::new_multi_thread()
            .enable_io()
            .enable_time()
            .thread_keep_alive(Duration::from_millis(1000))
            .worker_threads(2)
            .max_blocking_threads(2)
            .on_thread_start(|| {})
            .on_thread_stop(|| {})
            .build()
            .unwrap()
    })
    .block_on(async {
        actix_rt::time::sleep(Duration::from_millis(1)).await;

        tokio::task::spawn(async move {
            tx.send(42).unwrap();
        })
        .await
        .unwrap();

        123usize
    });

    assert_eq!(res, 123);
    assert_eq!(rx.recv().unwrap(), 42);
}

#[cfg(not(feature = "io-uring"))]
#[test]
fn new_arbiter_with_tokio() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let _ = System::new();

    let arb = Arbiter::with_tokio_rt(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    });

    let counter = Arc::new(AtomicBool::new(true));

    let counter1 = counter.clone();
    let did_spawn = arb.spawn(async move {
        actix_rt::time::sleep(Duration::from_millis(1)).await;
        counter1.store(false, Ordering::SeqCst);
        Arbiter::current().stop();
    });

    assert!(did_spawn);

    arb.join().unwrap();

    assert!(!counter.load(Ordering::SeqCst));
}

#[test]
#[should_panic]
fn no_system_current_panic() {
    System::current();
}

#[test]
#[should_panic]
fn no_system_arbiter_new_panic() {
    Arbiter::new();
}

#[test]
fn try_current_no_system() {
    assert!(System::try_current().is_none())
}

#[test]
fn try_current_with_system() {
    System::new().block_on(async { assert!(System::try_current().is_some()) });
}

#[allow(clippy::unit_cmp)]
#[test]
fn spawn_local() {
    System::new().block_on(async {
        // demonstrate that spawn -> R is strictly more capable than spawn -> ()

        assert_eq!(actix_rt::spawn(async {}).await.unwrap(), ());
        assert_eq!(actix_rt::spawn(async { 1 }).await.unwrap(), 1);
        assert!(actix_rt::spawn(async { panic!("") }).await.is_err());

        actix_rt::spawn(async { tokio::time::sleep(Duration::from_millis(50)).await })
            .await
            .unwrap();

        fn g<F: Future<Output = Result<(), JoinError>>>(_f: F) {}
        g(actix_rt::spawn(async {}));
        // g(actix_rt::spawn(async { 1 })); // compile err

        fn h<F: Future<Output = Result<R, JoinError>>, R>(_f: F) {}
        h(actix_rt::spawn(async {}));
        h(actix_rt::spawn(async { 1 }));
    })
}

#[cfg(all(target_os = "linux", feature = "io-uring"))]
#[test]
fn tokio_uring_arbiter() {
    System::new().block_on(async {
        let (tx, rx) = std::sync::mpsc::channel();

        Arbiter::new().spawn(async move {
            let handle = actix_rt::spawn(async move {
                let f = tokio_uring::fs::File::create("test.txt").await.unwrap();
                let buf = b"Hello World!";

                let (res, _) = f.write_at(&buf[..], 0).await;
                assert!(res.is_ok());

                f.sync_all().await.unwrap();
                f.close().await.unwrap();

                std::fs::remove_file("test.txt").unwrap();
            });

            handle.await.unwrap();
            tx.send(true).unwrap();
        });

        assert!(rx.recv().unwrap());
    })
}
