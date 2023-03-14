use std::error::Error;
use std::time::{Duration, Instant};

use futures_timer::Delay;

#[async_std::test]
async fn smoke() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let dur = Duration::from_millis(10);
    let start = Instant::now();
    Delay::new(dur).await;
    assert!(start.elapsed() >= (dur / 2));
    Ok(())
}

#[async_std::test]
async fn two() -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    let dur = Duration::from_millis(10);
    Delay::new(dur).await;
    Delay::new(dur).await;
    Ok(())
}
