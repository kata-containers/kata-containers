#[cfg(not(target_arch = "wasm32"))]
mod concurrent {
    use async_once::AsyncOnce;
    use core::future::Future;
    use futures::future;
    use lazy_static::lazy_static;
    use std::{
        pin::Pin,
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::runtime::Runtime;

    lazy_static! {
        static ref FOO: AsyncOnce<u32> = AsyncOnce::new(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            1
        });
    }

    /// this test triggers a deadlock the test never commpletes
    #[test]
    fn simultaneous_access() {
        let child = std::thread::spawn(|| {
            Runtime::new()
                .unwrap()
                .block_on(async { assert_eq!(FOO.get().await, &1) });
        });

        Runtime::new()
            .unwrap()
            .block_on(async { assert_eq!(FOO.get().await, &1) });

        child.join().unwrap();
    }

    struct Fut1 {
        i: u32,
    }

    impl Future for Fut1 {
        type Output = u32;
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
            self.i = self.i + 1;
            if self.i == 3 {
                Poll::Ready(self.i)
            } else {
                cx.waker().clone().wake();
                Poll::Pending
            }
        }
    }

    #[test]
    fn test_wake() -> Result<(), ()> {
        use tokio::runtime::Runtime;
        use tokio::task;

        lazy_static::lazy_static! { static ref ONCE : AsyncOnce<u32> = AsyncOnce::new(async {
            tokio::time::sleep(Duration::from_millis(1)).await;
            1
        });}
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let t2 = task::spawn(ONCE.get());
            let _child = task::spawn(async {
                let value = match future::select(Fut1 { i: 1 }, ONCE.get()).await {
                    future::Either::Left((value, _)) => value,
                    future::Either::Right((value, _)) => *value,
                };
                assert!(value == 3);
            });
            let value = t2.await.unwrap();
            assert!(value == &1);
        });
        Ok(())
    }

    #[test]
    fn wake_twice() -> Result<(), ()> {
        use tokio::runtime::Runtime;
        use tokio::task;

        lazy_static::lazy_static! { static ref ONCE : AsyncOnce<u32> = AsyncOnce::new(async {
            Fut1{i:0}.await;
            tokio::time::sleep(Duration::from_millis(10)).await;
            1
        });}
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let t2 = task::spawn(ONCE.get());

            let _child = task::spawn(async {
                let value = ONCE.get().await;
                assert!(value == &1);
            });
            let value = t2.await.unwrap();
            assert!(value == &1);
        });
        Ok(())
    }
}
