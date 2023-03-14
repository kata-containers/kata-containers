// TODO: support `SshTime` without `chrono` nor `time`
#[cfg(not(any(feature = "chrono_conversion", feature = "time_conversion")))]
compile_error!(
    "Either feature \"chrono_conversion\" or \"time_conversion\" must be enabled when the feature \"ssh\" is set."
);

pub use time_impl::SshTime;

#[cfg(feature = "time_conversion")]
mod time_impl {
    use time::OffsetDateTime;

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub struct SshTime(pub(crate) OffsetDateTime);

    impl SshTime {
        pub fn now() -> Self {
            Self(OffsetDateTime::now_utc())
        }

        pub fn from_timestamp(timestamp: u64) -> Self {
            Self(OffsetDateTime::from_unix_timestamp(timestamp as i64).unwrap())
        }

        pub fn timestamp(&self) -> u64 {
            self.0.unix_timestamp() as u64
        }

        pub fn month(&self) -> u8 {
            u8::from(self.0.month())
        }

        pub fn day(&self) -> u8 {
            self.0.day()
        }

        pub fn hour(&self) -> u8 {
            self.0.hour()
        }

        pub fn minute(&self) -> u8 {
            self.0.minute()
        }

        pub fn second(&self) -> u8 {
            self.0.second()
        }

        pub fn year(&self) -> u16 {
            self.0.year().try_into().unwrap()
        }
    }

    impl From<SshTime> for OffsetDateTime {
        fn from(time: SshTime) -> Self {
            time.0
        }
    }

    impl From<OffsetDateTime> for SshTime {
        fn from(time: OffsetDateTime) -> Self {
            Self::from_timestamp(time.unix_timestamp() as u64)
        }
    }

    impl From<SshTime> for u64 {
        fn from(time: SshTime) -> u64 {
            time.0.unix_timestamp() as u64
        }
    }
}

#[cfg(all(feature = "chrono_conversion", not(feature = "time_conversion")))]
mod time_impl {
    use chrono::{DateTime, Utc};
    pub use chrono::{Datelike, Timelike};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub struct SshTime(pub(crate) DateTime<Utc>);

    impl SshTime {
        pub fn now() -> Self {
            SshTime(DateTime::<Utc>::from(SystemTime::now()))
        }

        pub fn from_timestamp(timestamp: u64) -> Self {
            Self(DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(timestamp)))
        }

        pub fn timestamp(&self) -> u64 {
            self.0.timestamp() as u64
        }

        pub fn month(&self) -> u8 {
            self.0.month().try_into().unwrap()
        }

        pub fn day(&self) -> u8 {
            self.0.day().try_into().unwrap()
        }

        pub fn hour(&self) -> u8 {
            self.0.hour().try_into().unwrap()
        }

        pub fn minute(&self) -> u8 {
            self.0.minute().try_into().unwrap()
        }

        pub fn second(&self) -> u8 {
            self.0.second().try_into().unwrap()
        }

        pub fn year(&self) -> u16 {
            self.0.year().try_into().unwrap()
        }
    }

    impl From<DateTime<Utc>> for SshTime {
        fn from(date: DateTime<Utc>) -> Self {
            Self(date)
        }
    }

    impl From<SshTime> for DateTime<Utc> {
        fn from(time: SshTime) -> Self {
            time.0
        }
    }
}
