use std::error::Error;
use std::pin::Pin;
use std::time::{Duration, Instant};

use futures_timer::Delay;

#[async_std::test]
async fn works() {
    let i = Instant::now();
    let dur = Duration::from_millis(100);
    let _d = Delay::new(dur).await;
    assert!(i.elapsed() > dur);
}

#[async_std::test]
async fn reset() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let i = Instant::now();
    let dur = Duration::from_millis(100);
    let mut d = Delay::new(dur);

    // Allow us to re-use a future
    Pin::new(&mut d).await;

    assert!(i.elapsed() > dur);

    let i = Instant::now();
    d.reset(dur);
    d.await;
    assert!(i.elapsed() > dur);
    Ok(())
}
