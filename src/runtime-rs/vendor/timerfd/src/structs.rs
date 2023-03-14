use std::time::Duration;
use std::convert::TryInto;
use rustix::time::{Itimerspec, Timespec};

use TimerState;

const TS_NULL: Timespec = Timespec { tv_sec: 0, tv_nsec: 0 };

fn to_timespec(d: Duration) -> Timespec {
    // We don't need to check for overflow in the `nsec` conversion,
    // because `Duration` guarantees that `subsec_nanos()` is always
    // less than a billion, which will always fit into `tv_nsec`.
    Timespec {
        tv_sec: d.as_secs().try_into().unwrap(),
        tv_nsec: d.subsec_nanos() as _,
    }
}

fn from_timespec(ts: Timespec) -> Duration {
    // We don't need to check for overflow here, since these are only
    // used to convert `Timespec` values we get from the OS, which we
    // assume are valid.
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

impl From<TimerState> for Itimerspec {
    fn from(ts: TimerState) -> Itimerspec {
        match ts {
            TimerState::Disarmed => Itimerspec {
                it_value: TS_NULL,
                it_interval: TS_NULL
            },
            TimerState::Oneshot(d) => Itimerspec {
                it_value: to_timespec(d),
                it_interval: TS_NULL,
            },
            TimerState::Periodic { current, interval } => Itimerspec {
                it_value: to_timespec(current),
                it_interval: to_timespec(interval)
            },
        }
    }
}

impl From<Itimerspec> for TimerState {
    fn from(its: Itimerspec) -> TimerState {
        match its {
            Itimerspec { it_value, ..  } if it_value == TS_NULL => {
                TimerState::Disarmed
            }
            Itimerspec { it_value, it_interval } if it_interval == TS_NULL => {
                TimerState::Oneshot(from_timespec(it_value))
            }
            Itimerspec { it_value, it_interval } => {
                TimerState::Periodic {
                    current: from_timespec(it_value),
                    interval: from_timespec(it_interval)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_disarmed() {
        let start = TimerState::Disarmed;
        let clone = start.clone();
        assert_eq!(clone, start);
        let native: Itimerspec = clone.into();
        assert!(native.it_value.tv_sec == 0);
        assert!(native.it_value.tv_nsec == 0);

        let target: TimerState = native.into();
        assert_eq!(target, start);
    }

    #[test]
    fn convert_oneshot() {
        let start = TimerState::Oneshot(Duration::new(1, 0));
        let clone = start.clone();
        assert_eq!(clone, start);
        let native: Itimerspec = clone.into();
        assert!(native.it_interval.tv_sec == 0);
        assert!(native.it_interval.tv_nsec == 0);
        assert!(native.it_value.tv_sec == 1);
        assert!(native.it_value.tv_nsec == 0);

        let target: TimerState = native.into();
        assert_eq!(target, start);
    }

    #[test]
    fn convert_periodic() {
        let start = TimerState::Periodic {
            current: Duration::new(1, 0),
            interval: Duration::new(0, 1),
        };
        let clone = start.clone();
        assert_eq!(clone, start);
        let native: Itimerspec = clone.into();
        assert!(native.it_interval.tv_sec == 0);
        assert!(native.it_interval.tv_nsec == 1);
        assert!(native.it_value.tv_sec == 1);
        assert!(native.it_value.tv_nsec == 0);

        let target: TimerState = native.into();
        assert_eq!(target, start);
    }
}
