#[test]
fn mutex_acquire_uncontested() {
    use futures::future::FutureExt;
    use futures::lock::Mutex;
    use futures_test::task::panic_context;

    let mutex = Mutex::new(());
    for _ in 0..10 {
        assert!(mutex.lock().poll_unpin(&mut panic_context()).is_ready());
    }
}

#[test]
fn mutex_wakes_waiters() {
    use futures::future::FutureExt;
    use futures::lock::Mutex;
    use futures::task::Context;
    use futures_test::task::{new_count_waker, panic_context};

    let mutex = Mutex::new(());
    let (waker, counter) = new_count_waker();
    let lock = mutex.lock().poll_unpin(&mut panic_context());
    assert!(lock.is_ready());

    let mut cx = Context::from_waker(&waker);
    let mut waiter = mutex.lock();
    assert!(waiter.poll_unpin(&mut cx).is_pending());
    assert_eq!(counter, 0);

    drop(lock);

    assert_eq!(counter, 1);
    assert!(waiter.poll_unpin(&mut panic_context()).is_ready());
}

#[test]
fn mutex_contested() {
    use futures::channel::mpsc;
    use futures::executor::block_on;
    use futures::future::ready;
    use futures::lock::Mutex;
    use futures::stream::StreamExt;
    use futures::task::SpawnExt;
    use futures_test::future::FutureTestExt;
    use std::sync::Arc;

    let (tx, mut rx) = mpsc::unbounded();
    let pool = futures::executor::ThreadPool::builder()
        .pool_size(16)
        .create()
        .unwrap();

    let tx = Arc::new(tx);
    let mutex = Arc::new(Mutex::new(0));

    let num_tasks = 1000;
    for _ in 0..num_tasks {
        let tx = tx.clone();
        let mutex = mutex.clone();
        pool.spawn(async move {
            let mut lock = mutex.lock().await;
            ready(()).pending_once().await;
            *lock += 1;
            tx.unbounded_send(()).unwrap();
            drop(lock);
        })
        .unwrap();
    }

    block_on(async {
        for _ in 0..num_tasks {
            rx.next().await.unwrap();
        }
        let lock = mutex.lock().await;
        assert_eq!(num_tasks, *lock);
    })
}
