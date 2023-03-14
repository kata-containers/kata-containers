#[cfg(not(feature = "debug_log"))]
macro_rules! debug_log {
    () => {};
    ($($arg:tt)*) => {};
}

#[cfg(feature = "debug_log")]
#[macro_use]
pub mod internal {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::thread::ThreadId;

    lazy_static::lazy_static! {
        pub static ref CTX: Mutex<HashMap<ThreadId, u8>> = Mutex::new(HashMap::new());
    }

    pub struct Identer;
    impl Identer {
        pub fn ident() -> Identer {
            CTX.lock()
                .unwrap()
                .entry(::std::thread::current().id())
                .and_modify(|c| *c += 1)
                .or_insert(1);
            Self
        }
    }

    impl Drop for Identer {
        fn drop(&mut self) {
            CTX.lock()
                .unwrap()
                .entry(::std::thread::current().id())
                .and_modify(|c| *c -= 1);
        }
    }

    macro_rules! debug_log {
        () => {
            println!("| debug  |");
        };
        ($($arg:tt)*) => {
            let indent = *$crate::debug_log::internal::CTX.lock()
                .unwrap()
                .get(&::std::thread::current().id())
                .unwrap_or(&0);
            let mut blanks = String::with_capacity(indent as usize);
            for _ in 0..indent {
                blanks.push_str("| ");
            }

            print!("| debug => {}", blanks);
            println!($($arg)*);
            let _identer = $crate::debug_log::internal::Identer::ident();
        };
    }
}
