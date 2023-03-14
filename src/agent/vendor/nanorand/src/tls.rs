use crate::rand::{wyrand::WyRand, Rng, SeedableRng};
use std::{cell::RefCell, rc::Rc};

thread_local! {
	static WYRAND: Rc<RefCell<WyRand>> = Rc::new(RefCell::new(WyRand::new()));
}

#[derive(Clone)]
#[doc(hidden)]
pub struct TlsWyRand(Rc<RefCell<WyRand>>);

impl Rng<8> for TlsWyRand {
	fn rand(&mut self) -> [u8; 8] {
		self.0.borrow_mut().rand()
	}
}

impl SeedableRng<8, 8> for TlsWyRand {
	fn reseed(&mut self, seed: [u8; 8]) {
		self.0.borrow_mut().reseed(seed);
	}
}

/// Fetch a thread-local [`WyRand`]
/// ```rust
/// use nanorand::Rng;
///
/// let mut rng = nanorand::tls_rng();
/// println!("Random number: {}", rng.generate::<u64>());
/// ```
/// This cannot be passed to another thread, as something like this will fail to compile:
/// ```compile_fail
/// use nanorand::Rng;
///
/// let mut rng = nanorand::tls_rng();
/// std::thread::spawn(move || {
///     println!("Random number: {}", rng.generate::<u64>());
/// });
/// ```
pub fn tls_rng() -> TlsWyRand {
	WYRAND.with(|tls| TlsWyRand(tls.clone()))
}
