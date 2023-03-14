// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{cell::RefCell, env, fs, time::SystemTime};

use super::tz_info::TimeZone;
use super::{DateTime, FixedOffset, Local, NaiveDateTime};
use crate::{Datelike, LocalResult, Utc};

pub(super) fn now() -> DateTime<Local> {
    let now = Utc::now().naive_utc();
    naive_to_local(&now, false).unwrap()
}

pub(super) fn naive_to_local(d: &NaiveDateTime, local: bool) -> LocalResult<DateTime<Local>> {
    TZ_INFO.with(|maybe_cache| {
        maybe_cache.borrow_mut().get_or_insert_with(Cache::default).offset(*d, local)
    })
}

// we have to store the `Cache` in an option as it can't
// be initalized in a static context.
thread_local! {
    static TZ_INFO: RefCell<Option<Cache>> = Default::default();
}

enum Source {
    LocalTime { mtime: SystemTime, last_checked: SystemTime },
    // we don't bother storing the contents of the environment variable in this case.
    // changing the environment while the process is running is generally not reccomended
    Environment,
}

impl Default for Source {
    fn default() -> Source {
        // use of var_os avoids allocating, which is nice
        // as we are only going to discard the string anyway
        // but we must ensure the contents are valid unicode
        // otherwise the behaivour here would be different
        // to that in `naive_to_local`
        match env::var_os("TZ") {
            Some(ref s) if s.to_str().is_some() => Source::Environment,
            Some(_) | None => match fs::symlink_metadata("/etc/localtime") {
                Ok(data) => Source::LocalTime {
                    // we have to pick a sensible default when the mtime fails
                    // by picking SystemTime::now() we raise the probability of
                    // the cache being invalidated if/when the mtime starts working
                    mtime: data.modified().unwrap_or_else(|_| SystemTime::now()),
                    last_checked: SystemTime::now(),
                },
                Err(_) => {
                    // as above, now() should be a better default than some constant
                    // TODO: see if we can improve caching in the case where the fallback is a valid timezone
                    Source::LocalTime { mtime: SystemTime::now(), last_checked: SystemTime::now() }
                }
            },
        }
    }
}

impl Source {
    fn out_of_date(&mut self) -> bool {
        let now = SystemTime::now();
        let prev = match self {
            Source::LocalTime { mtime, last_checked } => match now.duration_since(*last_checked) {
                Ok(d) if d.as_secs() < 1 => return false,
                Ok(_) | Err(_) => *mtime,
            },
            Source::Environment => return false,
        };

        match Source::default() {
            Source::LocalTime { mtime, .. } => {
                *self = Source::LocalTime { mtime, last_checked: now };
                prev != mtime
            }
            // will only reach here if TZ has been set while
            // the process is running
            Source::Environment => {
                *self = Source::Environment;
                true
            }
        }
    }
}

struct Cache {
    zone: TimeZone,
    source: Source,
}

#[cfg(target_os = "android")]
const TZDB_LOCATION: &str = " /system/usr/share/zoneinfo";

#[allow(dead_code)] // keeps the cfg simpler
#[cfg(not(target_os = "android"))]
const TZDB_LOCATION: &str = "/usr/share/zoneinfo";

fn fallback_timezone() -> Option<TimeZone> {
    let tz_name = iana_time_zone::get_timezone().ok()?;
    let bytes = fs::read(format!("{}/{}", TZDB_LOCATION, tz_name)).ok()?;
    TimeZone::from_tz_data(&bytes).ok()
}

impl Default for Cache {
    fn default() -> Cache {
        // default to UTC if no local timezone can be found
        Cache {
            zone: TimeZone::local().ok().or_else(fallback_timezone).unwrap_or_else(TimeZone::utc),
            source: Source::default(),
        }
    }
}

impl Cache {
    fn offset(&mut self, d: NaiveDateTime, local: bool) -> LocalResult<DateTime<Local>> {
        if self.source.out_of_date() {
            *self = Cache::default();
        }

        if !local {
            let offset = FixedOffset::east(
                self.zone
                    .find_local_time_type(d.timestamp())
                    .expect("unable to select local time type")
                    .offset(),
            );

            return LocalResult::Single(DateTime::from_utc(d, offset));
        }

        // we pass through the year as the year of a local point in time must either be valid in that locale, or
        // the entire time was skipped in which case we will return LocalResult::None anywa.
        match self
            .zone
            .find_local_time_type_from_local(d.timestamp(), d.year())
            .expect("unable to select local time type")
        {
            LocalResult::None => LocalResult::None,
            LocalResult::Ambiguous(early, late) => {
                let early_offset = FixedOffset::east(early.offset());
                let late_offset = FixedOffset::east(late.offset());

                LocalResult::Ambiguous(
                    DateTime::from_utc(d - early_offset, early_offset),
                    DateTime::from_utc(d - late_offset, late_offset),
                )
            }
            LocalResult::Single(tt) => {
                let offset = FixedOffset::east(tt.offset());
                LocalResult::Single(DateTime::from_utc(d - offset, offset))
            }
        }
    }
}
