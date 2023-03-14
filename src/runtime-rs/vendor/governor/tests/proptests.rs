use proptest::prelude::*;

use governor::{
    clock::{Clock, FakeRelativeClock},
    Quota, RateLimiter,
};
use proptest::prelude::prop::test_runner::FileFailurePersistence;
use std::num::NonZeroU32;
use std::time::Duration;

#[derive(Debug)]
struct Count(NonZeroU32);
impl Arbitrary for Count {
    type Parameters = ();
    fn arbitrary_with(_args: ()) -> Self::Strategy {
        (1..10000u32)
            .prop_map(|x| Count(NonZeroU32::new(x).unwrap()))
            .boxed()
    }

    type Strategy = BoxedStrategy<Count>;
}

fn test_config() -> ProptestConfig {
    let mut cfg = ProptestConfig::default();
    cfg.failure_persistence = Some(Box::new(FileFailurePersistence::WithSource("regressions")));
    //cfg.timeout = 20;
    cfg.verbose = 0; // 2 for extra verbosity;
    cfg
}

#[cfg(feature = "std")]
#[test]
fn cover_count_derives() {
    use nonzero_ext::nonzero;
    let count = Count(nonzero!(1u32));
    assert_eq!(format!("{:?}", count), "Count(1)");
}

#[test]
fn accurate_not_until() {
    proptest!(test_config(), |(capacity: Count, additional: Count, wait_time_parts: Count)| {
        let clock = FakeRelativeClock::default();
        let lb = RateLimiter::direct_with_clock(Quota::per_second(capacity.0), &clock);
        let step = Duration::from_secs(1) / capacity.0.get();

        // use up the burst capacity:
        for _ in 0..capacity.0.get() {
            prop_assert!(lb.check().is_ok());
        }

        // step forward a few times:
        for _ in 0..additional.0.get() {
            clock.advance(step);
            prop_assert!(lb.check().is_ok());
        }

        // get a negative response:
        if let Err(negative) = lb.check() {
            // check in steps up until the expected time to see whether we are allowed
            let fractions = wait_time_parts.0.get();
            let remaining_ns = negative.wait_time_from(clock.now());
            let wait_time = remaining_ns / (fractions+1);

            for i in 0..fractions {
                clock.advance(wait_time);
                prop_assert_ne!(Ok(()), lb.check(),
                           "Got a positive response after {:?} steps on {:?} from {:?} at {:?}",
                           i, remaining_ns, lb, clock.now());
            }
            // should be ok exactly where it tells us:
            clock.advance(negative.wait_time_from(clock.now()));
            prop_assert_eq!(Ok(()), lb.check(),
                       "Got a negative response from {:?} at {:?}",
                       lb, clock.now());
        } else {
            prop_assert!(false, "got a positive response after exhausting the limiter");
        }
    });
}
