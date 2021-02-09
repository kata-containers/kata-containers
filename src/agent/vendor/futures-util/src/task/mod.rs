//! Task notification

cfg_target_has_atomic! {
    #[cfg(feature = "alloc")]
    pub use futures_task::ArcWake;

    #[cfg(feature = "alloc")]
    pub use futures_task::waker;

    #[cfg(feature = "alloc")]
    pub use futures_task::{waker_ref, WakerRef};

    pub use futures_core::task::__internal::AtomicWaker;
}

mod spawn;
pub use self::spawn::{SpawnExt, LocalSpawnExt};

pub use futures_core::task::{Context, Poll, Waker, RawWaker, RawWakerVTable};

pub use futures_task::{
    Spawn, LocalSpawn, SpawnError,
    FutureObj, LocalFutureObj, UnsafeFutureObj,
};

pub use futures_task::noop_waker;
#[cfg(feature = "std")]
pub use futures_task::noop_waker_ref;
