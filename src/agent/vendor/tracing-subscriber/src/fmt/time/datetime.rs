use std::fmt;

/// A date/time type which exists primarily to convert `SystemTime` timestamps into an ISO 8601
/// formatted string.
///
/// Yes, this exists. Before you have a heart attack, understand that the meat of this is musl's
/// [`__secs_to_tm`][1] converted to Rust via [c2rust][2] and then cleaned up by hand. All existing
/// `strftime`-like APIs I found were unable to handle the full range of timestamps representable
/// by `SystemTime`, including `strftime` itself, since tm.tm_year is an int.
///
/// TODO: figure out how to properly attribute the MIT licensed musl project.
///
/// [1] http://git.musl-libc.org/cgit/musl/tree/src/time/__secs_to_tm.c
/// [2] https://c2rust.com/
///
/// This is directly copy-pasted from https://github.com/danburkert/kudu-rs/blob/c9660067e5f4c1a54143f169b5eeb49446f82e54/src/timestamp.rs#L5-L18
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DateTime {
    year: i64,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanos: u32,
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.year > 9999 {
            write!(f, "+{}", self.year)?;
        } else if self.year < 0 {
            write!(f, "{:05}", self.year)?;
        } else {
            write!(f, "{:04}", self.year)?;
        }

        write!(
            f,
            "-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            self.nanos / 1_000
        )
    }
}

impl From<std::time::SystemTime> for DateTime {
    fn from(timestamp: std::time::SystemTime) -> DateTime {
        let (t, nanos) = match timestamp.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => {
                debug_assert!(duration.as_secs() <= std::i64::MAX as u64);
                (duration.as_secs() as i64, duration.subsec_nanos())
            }
            Err(error) => {
                let duration = error.duration();
                debug_assert!(duration.as_secs() <= std::i64::MAX as u64);
                let (secs, nanos) = (duration.as_secs() as i64, duration.subsec_nanos());
                if nanos == 0 {
                    (-secs, 0)
                } else {
                    (-secs - 1, 1_000_000_000 - nanos)
                }
            }
        };

        // 2000-03-01 (mod 400 year, immediately after feb29
        const LEAPOCH: i64 = 946_684_800 + 86400 * (31 + 29);
        const DAYS_PER_400Y: i32 = 365 * 400 + 97;
        const DAYS_PER_100Y: i32 = 365 * 100 + 24;
        const DAYS_PER_4Y: i32 = 365 * 4 + 1;
        static DAYS_IN_MONTH: [i8; 12] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];

        // Note(dcb): this bit is rearranged slightly to avoid integer overflow.
        let mut days: i64 = (t / 86_400) - (LEAPOCH / 86_400);
        let mut remsecs: i32 = (t % 86_400) as i32;
        if remsecs < 0i32 {
            remsecs += 86_400;
            days -= 1
        }

        let mut qc_cycles: i32 = (days / i64::from(DAYS_PER_400Y)) as i32;
        let mut remdays: i32 = (days % i64::from(DAYS_PER_400Y)) as i32;
        if remdays < 0 {
            remdays += DAYS_PER_400Y;
            qc_cycles -= 1;
        }

        let mut c_cycles: i32 = remdays / DAYS_PER_100Y;
        if c_cycles == 4 {
            c_cycles -= 1;
        }
        remdays -= c_cycles * DAYS_PER_100Y;

        let mut q_cycles: i32 = remdays / DAYS_PER_4Y;
        if q_cycles == 25 {
            q_cycles -= 1;
        }
        remdays -= q_cycles * DAYS_PER_4Y;

        let mut remyears: i32 = remdays / 365;
        if remyears == 4 {
            remyears -= 1;
        }
        remdays -= remyears * 365;

        let mut years: i64 = i64::from(remyears)
            + 4 * i64::from(q_cycles)
            + 100 * i64::from(c_cycles)
            + 400 * i64::from(qc_cycles);

        let mut months: i32 = 0;
        while i32::from(DAYS_IN_MONTH[months as usize]) <= remdays {
            remdays -= i32::from(DAYS_IN_MONTH[months as usize]);
            months += 1
        }

        if months >= 10 {
            months -= 12;
            years += 1;
        }

        DateTime {
            year: years + 2000,
            month: (months + 3) as u8,
            day: (remdays + 1) as u8,
            hour: (remsecs / 3600) as u8,
            minute: (remsecs / 60 % 60) as u8,
            second: (remsecs % 60) as u8,
            nanos,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::i32;
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    #[test]
    fn test_datetime() {
        let case = |expected: &str, secs: i64, micros: u32| {
            let timestamp = if secs >= 0 {
                UNIX_EPOCH + Duration::new(secs as u64, micros * 1_000)
            } else {
                (UNIX_EPOCH - Duration::new(!secs as u64 + 1, 0)) + Duration::new(0, micros * 1_000)
            };
            assert_eq!(
                expected,
                format!("{}", DateTime::from(timestamp)),
                "secs: {}, micros: {}",
                secs,
                micros
            )
        };

        // Mostly generated with:
        //  - date -jur <secs> +"%Y-%m-%dT%H:%M:%S.000000Z"
        //  - http://unixtimestamp.50x.eu/

        case("1970-01-01T00:00:00.000000Z", 0, 0);

        case("1970-01-01T00:00:00.000001Z", 0, 1);
        case("1970-01-01T00:00:00.500000Z", 0, 500_000);
        case("1970-01-01T00:00:01.000001Z", 1, 1);
        case("1970-01-01T00:01:01.000001Z", 60 + 1, 1);
        case("1970-01-01T01:01:01.000001Z", 60 * 60 + 60 + 1, 1);
        case(
            "1970-01-02T01:01:01.000001Z",
            24 * 60 * 60 + 60 * 60 + 60 + 1,
            1,
        );

        case("1969-12-31T23:59:59.000000Z", -1, 0);
        case("1969-12-31T23:59:59.000001Z", -1, 1);
        case("1969-12-31T23:59:59.500000Z", -1, 500_000);
        case("1969-12-31T23:58:59.000001Z", -60 - 1, 1);
        case("1969-12-31T22:58:59.000001Z", -60 * 60 - 60 - 1, 1);
        case(
            "1969-12-30T22:58:59.000001Z",
            -24 * 60 * 60 - 60 * 60 - 60 - 1,
            1,
        );

        case("2038-01-19T03:14:07.000000Z", std::i32::MAX as i64, 0);
        case("2038-01-19T03:14:08.000000Z", std::i32::MAX as i64 + 1, 0);
        case("1901-12-13T20:45:52.000000Z", i32::MIN as i64, 0);
        case("1901-12-13T20:45:51.000000Z", i32::MIN as i64 - 1, 0);

        // Skipping these tests on windows as std::time::SysteTime range is low
        // on Windows compared with that of Unix which can cause the following
        // high date value tests to panic
        #[cfg(not(target_os = "windows"))]
        {
            case("+292277026596-12-04T15:30:07.000000Z", std::i64::MAX, 0);
            case("+292277026596-12-04T15:30:06.000000Z", std::i64::MAX - 1, 0);
            case("-292277022657-01-27T08:29:53.000000Z", i64::MIN + 1, 0);
        }

        case("1900-01-01T00:00:00.000000Z", -2208988800, 0);
        case("1899-12-31T23:59:59.000000Z", -2208988801, 0);
        case("0000-01-01T00:00:00.000000Z", -62167219200, 0);
        case("-0001-12-31T23:59:59.000000Z", -62167219201, 0);

        case("1234-05-06T07:08:09.000000Z", -23215049511, 0);
        case("-1234-05-06T07:08:09.000000Z", -101097651111, 0);
        case("2345-06-07T08:09:01.000000Z", 11847456541, 0);
        case("-2345-06-07T08:09:01.000000Z", -136154620259, 0);
    }
}
