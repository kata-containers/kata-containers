use std::env;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

static LOCK: AtomicBool = AtomicBool::new(false);

/// Current working directory.
#[doc(hidden)]
pub struct CWD {
    path: Option<Rc<PathBuf>>,
}

impl CWD {
    #[inline]
    pub(crate) const fn new() -> CWD {
        CWD {
            path: None,
        }
    }

    #[inline]
    pub(crate) fn update(&mut self) -> Option<Rc<PathBuf>> {
        if LOCK.compare_and_swap(true, false, Ordering::Relaxed) {
            let cwd = Rc::new(env::current_dir().unwrap());

            self.path.replace(cwd.clone());

            LOCK.store(true, Ordering::Relaxed);

            Some(cwd)
        } else {
            None
        }
    }

    #[inline]
    #[doc(hidden)]
    pub fn initial(&mut self) -> Rc<PathBuf> {
        match self.path.as_ref() {
            Some(path) => path.clone(),
            None => {
                match self.update() {
                    Some(path) => path,
                    None => Rc::new(env::current_dir().unwrap()),
                }
            }
        }
    }
}

impl Deref for CWD {
    type Target = Path;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.path.as_deref().unwrap()
    }
}
