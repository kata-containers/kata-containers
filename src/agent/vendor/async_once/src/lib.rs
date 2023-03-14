//! ## async once tool for lazy_static
//!
//! # Examples
//! ```rust
//!    use lazy_static::lazy_static;
//!    use tokio::runtime::Builder;
//!    use async_once::AsyncOnce;
//!
//!    lazy_static!{
//!        static ref FOO : AsyncOnce<u32> = AsyncOnce::new(async{
//!            1
//!        });
//!    }
//!    let rt = Builder::new_current_thread().build().unwrap();
//!    rt.block_on(async {
//!        assert_eq!(FOO.get().await , &1)
//!    })
//! ```
//!
//! ### run tests
//!
//! ```bash
//!    cargo test
//!    wasm-pack test --headless --chrome --firefox
//! ```
//!

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::ptr::null;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;

type Fut<T> = Mutex<Result<T, Pin<Box<dyn Future<Output = T>>>>>;
pub struct AsyncOnce<T: 'static> {
    ptr: Cell<*const T>,
    fut: Fut<T>,
    waker: Arc<MyWaker>,
}

unsafe impl<T: 'static> Sync for AsyncOnce<T> {}

impl<T> AsyncOnce<T> {
    pub fn new<F>(fut: F) -> AsyncOnce<T>
    where
        F: Future<Output = T> + 'static,
    {
        AsyncOnce {
            ptr: Cell::new(null()),
            fut: Mutex::new(Err(Box::pin(fut))),
            waker: Arc::new(MyWaker {
                wakers: Mutex::new(Vec::with_capacity(16)),
            }),
        }
    }
    #[inline(always)]
    pub fn get(&'static self) -> &'static Self {
        self
    }
}

struct MyWaker {
    wakers: Mutex<Vec<Waker>>,
}

impl Wake for MyWaker {
    fn wake_by_ref(self: &std::sync::Arc<Self>) {
        self.clone().wake();
    }

    fn wake(self: std::sync::Arc<Self>) {
        let mut wakers = self.wakers.lock().unwrap();
        while let Some(waker) = wakers.pop() {
            waker.wake();
        }
        drop(wakers);
    }
}

impl<T> Future for &'static AsyncOnce<T> {
    type Output = &'static T;
    #[inline(always)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<&'static T> {
        if let Some(ptr) = unsafe { self.ptr.get().as_ref() } {
            return Poll::Ready(ptr);
        }
        let cxwaker = cx.waker().clone();
        let mut wakers = self.waker.wakers.lock().unwrap();
        let is_first = wakers.is_empty();
        if !wakers.iter().any(|wk| wk.will_wake(&cxwaker)) {
            wakers.push(cxwaker);
        }
        drop(wakers);
        let mut result = None;
        let mut fut = self.fut.lock().unwrap();
        match (is_first, fut.as_mut()) {
            (true, Err(fut)) => {
                let waker = Waker::from(self.waker.clone());
                let mut ctx = Context::from_waker(&waker);
                match Pin::new(fut).poll(&mut ctx) {
                    Poll::Ready(res) => {
                        result = Some(res);
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            (true, Ok(res)) => {
                return Poll::Ready(unsafe { (res as *const T).as_ref().unwrap() });
            }
            _ => (),
        }
        if let Some(res) = result {
            *fut = Ok(res);
            let ptr = fut.as_ref().ok().unwrap() as *const T;
            self.ptr.set(ptr);
            drop(fut);
            let mut wakers = self.waker.wakers.lock().unwrap();
            while let Some(waker) = wakers.pop() {
                waker.wake();
            }
            drop(wakers);
            return Poll::Ready(unsafe { &*ptr });
        }
        drop(fut);
        Poll::Pending
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lazy_static_test_for_tokio() {
    use futures_timer::Delay;
    use lazy_static::lazy_static;
    use std::time::Duration;
    use tokio::runtime::Builder;
    lazy_static! {
        static ref FOO: AsyncOnce<u32> = AsyncOnce::new(async {
            tokio::spawn(async { assert_eq!(FOO.get().await, &1) });
            Delay::new(Duration::from_millis(100)).await;
            1
        });
    }
    let rt = Builder::new_current_thread().build().unwrap();
    let handle1 = rt.spawn(async {
        Delay::new(Duration::from_millis(100)).await;
        assert_eq!(FOO.get().await, &1)
    });
    let handle2 = rt.spawn(async {
        Delay::new(Duration::from_millis(150)).await;
        assert_eq!(FOO.get().await, &1)
    });
    rt.block_on(async {
        use futures::future;
        Delay::new(Duration::from_millis(50)).await;
        let value = match future::select(FOO.get(), future::ready(1u32)).await {
            future::Either::Left((value, _)) => *value,
            future::Either::Right((value, _)) => value,
        };
        assert_eq!(&value, &1);
        let _ = handle1.await;
        let _ = handle2.await;
    });
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lazy_static_test_for_async_std() {
    use async_std::task;
    use futures_timer::Delay;
    use lazy_static::lazy_static;
    use std::time::Duration;
    lazy_static! {
        static ref FOO: AsyncOnce<u32> = AsyncOnce::new(async {
            Delay::new(Duration::from_millis(100)).await;
            1
        });
    }
    task::spawn(async { assert_eq!(FOO.get().await, &1) });
    task::spawn(async { assert_eq!(FOO.get().await, &1) });
    task::spawn(async { assert_eq!(FOO.get().await, &1) });
    task::block_on(async {
        Delay::new(Duration::from_millis(200)).await;
        assert_eq!(FOO.get().await, &1);
    });
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn lazy_static_test_for_smol() {
    use futures_timer::Delay;
    use lazy_static::lazy_static;
    use std::time::Duration;
    lazy_static! {
        static ref FOO: AsyncOnce<u32> = AsyncOnce::new(async {
            Delay::new(Duration::from_millis(100)).await;
            1
        });
    }
    smol::spawn(async { assert_eq!(FOO.get().await, &1) }).detach();
    smol::spawn(async { assert_eq!(FOO.get().await, &1) }).detach();
    smol::spawn(async { assert_eq!(FOO.get().await, &1) }).detach();
    smol::block_on(async {
        Delay::new(Duration::from_millis(200)).await;
        assert_eq!(FOO.get().await, &1);
    });
}
