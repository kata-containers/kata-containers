use std::time::{SystemTime, UNIX_EPOCH};

use super::DateTime;
use crate::naive::{NaiveDate, NaiveTime};
#[cfg(feature = "clock")]
use crate::offset::Local;
use crate::offset::{FixedOffset, TimeZone, Utc};
use crate::oldtime::Duration;
#[cfg(feature = "clock")]
use crate::Datelike;

#[test]
fn test_datetime_offset() {
    let est = FixedOffset::west(5 * 60 * 60);
    let edt = FixedOffset::west(4 * 60 * 60);
    let kst = FixedOffset::east(9 * 60 * 60);

    assert_eq!(format!("{}", Utc.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06 07:08:09 UTC");
    assert_eq!(format!("{}", edt.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06 07:08:09 -04:00");
    assert_eq!(format!("{}", kst.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06 07:08:09 +09:00");
    assert_eq!(format!("{:?}", Utc.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06T07:08:09Z");
    assert_eq!(format!("{:?}", edt.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06T07:08:09-04:00");
    assert_eq!(format!("{:?}", kst.ymd(2014, 5, 6).and_hms(7, 8, 9)), "2014-05-06T07:08:09+09:00");

    // edge cases
    assert_eq!(format!("{:?}", Utc.ymd(2014, 5, 6).and_hms(0, 0, 0)), "2014-05-06T00:00:00Z");
    assert_eq!(format!("{:?}", edt.ymd(2014, 5, 6).and_hms(0, 0, 0)), "2014-05-06T00:00:00-04:00");
    assert_eq!(format!("{:?}", kst.ymd(2014, 5, 6).and_hms(0, 0, 0)), "2014-05-06T00:00:00+09:00");
    assert_eq!(format!("{:?}", Utc.ymd(2014, 5, 6).and_hms(23, 59, 59)), "2014-05-06T23:59:59Z");
    assert_eq!(
        format!("{:?}", edt.ymd(2014, 5, 6).and_hms(23, 59, 59)),
        "2014-05-06T23:59:59-04:00"
    );
    assert_eq!(
        format!("{:?}", kst.ymd(2014, 5, 6).and_hms(23, 59, 59)),
        "2014-05-06T23:59:59+09:00"
    );

    let dt = Utc.ymd(2014, 5, 6).and_hms(7, 8, 9);
    assert_eq!(dt, edt.ymd(2014, 5, 6).and_hms(3, 8, 9));
    assert_eq!(dt + Duration::seconds(3600 + 60 + 1), Utc.ymd(2014, 5, 6).and_hms(8, 9, 10));
    assert_eq!(
        dt.signed_duration_since(edt.ymd(2014, 5, 6).and_hms(10, 11, 12)),
        Duration::seconds(-7 * 3600 - 3 * 60 - 3)
    );

    assert_eq!(*Utc.ymd(2014, 5, 6).and_hms(7, 8, 9).offset(), Utc);
    assert_eq!(*edt.ymd(2014, 5, 6).and_hms(7, 8, 9).offset(), edt);
    assert!(*edt.ymd(2014, 5, 6).and_hms(7, 8, 9).offset() != est);
}

#[test]
fn test_datetime_date_and_time() {
    let tz = FixedOffset::east(5 * 60 * 60);
    let d = tz.ymd(2014, 5, 6).and_hms(7, 8, 9);
    assert_eq!(d.time(), NaiveTime::from_hms(7, 8, 9));
    assert_eq!(d.date(), tz.ymd(2014, 5, 6));
    assert_eq!(d.date().naive_local(), NaiveDate::from_ymd(2014, 5, 6));
    assert_eq!(d.date().and_time(d.time()), Some(d));

    let tz = FixedOffset::east(4 * 60 * 60);
    let d = tz.ymd(2016, 5, 4).and_hms(3, 2, 1);
    assert_eq!(d.time(), NaiveTime::from_hms(3, 2, 1));
    assert_eq!(d.date(), tz.ymd(2016, 5, 4));
    assert_eq!(d.date().naive_local(), NaiveDate::from_ymd(2016, 5, 4));
    assert_eq!(d.date().and_time(d.time()), Some(d));

    let tz = FixedOffset::west(13 * 60 * 60);
    let d = tz.ymd(2017, 8, 9).and_hms(12, 34, 56);
    assert_eq!(d.time(), NaiveTime::from_hms(12, 34, 56));
    assert_eq!(d.date(), tz.ymd(2017, 8, 9));
    assert_eq!(d.date().naive_local(), NaiveDate::from_ymd(2017, 8, 9));
    assert_eq!(d.date().and_time(d.time()), Some(d));

    let utc_d = Utc.ymd(2017, 8, 9).and_hms(12, 34, 56);
    assert!(utc_d < d);
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_with_timezone() {
    let local_now = Local::now();
    let utc_now = local_now.with_timezone(&Utc);
    let local_now2 = utc_now.with_timezone(&Local);
    assert_eq!(local_now, local_now2);
}

#[test]
fn test_datetime_rfc2822_and_rfc3339() {
    let edt = FixedOffset::east(5 * 60 * 60);
    assert_eq!(
        Utc.ymd(2015, 2, 18).and_hms(23, 16, 9).to_rfc2822(),
        "Wed, 18 Feb 2015 23:16:09 +0000"
    );
    assert_eq!(Utc.ymd(2015, 2, 18).and_hms(23, 16, 9).to_rfc3339(), "2015-02-18T23:16:09+00:00");
    assert_eq!(
        edt.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150).to_rfc2822(),
        "Wed, 18 Feb 2015 23:16:09 +0500"
    );
    assert_eq!(
        edt.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150).to_rfc3339(),
        "2015-02-18T23:16:09.150+05:00"
    );
    assert_eq!(
        edt.ymd(2015, 2, 18).and_hms_micro(23, 59, 59, 1_234_567).to_rfc2822(),
        "Wed, 18 Feb 2015 23:59:60 +0500"
    );
    assert_eq!(
        edt.ymd(2015, 2, 18).and_hms_micro(23, 59, 59, 1_234_567).to_rfc3339(),
        "2015-02-18T23:59:60.234567+05:00"
    );

    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 +0000"),
        Ok(FixedOffset::east(0).ymd(2015, 2, 18).and_hms(23, 16, 9))
    );
    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:16:09 -0000"),
        Ok(FixedOffset::east(0).ymd(2015, 2, 18).and_hms(23, 16, 9))
    );
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:16:09Z"),
        Ok(FixedOffset::east(0).ymd(2015, 2, 18).and_hms(23, 16, 9))
    );
    assert_eq!(
        DateTime::parse_from_rfc2822("Wed, 18 Feb 2015 23:59:60 +0500"),
        Ok(edt.ymd(2015, 2, 18).and_hms_milli(23, 59, 59, 1_000))
    );
    assert!(DateTime::parse_from_rfc2822("31 DEC 262143 23:59 -2359").is_err());
    assert_eq!(
        DateTime::parse_from_rfc3339("2015-02-18T23:59:60.234567+05:00"),
        Ok(edt.ymd(2015, 2, 18).and_hms_micro(23, 59, 59, 1_234_567))
    );
}

#[test]
fn test_rfc3339_opts() {
    use crate::SecondsFormat::*;
    let pst = FixedOffset::east(8 * 60 * 60);
    let dt = pst.ymd(2018, 1, 11).and_hms_nano(10, 5, 13, 84_660_000);
    assert_eq!(dt.to_rfc3339_opts(Secs, false), "2018-01-11T10:05:13+08:00");
    assert_eq!(dt.to_rfc3339_opts(Secs, true), "2018-01-11T10:05:13+08:00");
    assert_eq!(dt.to_rfc3339_opts(Millis, false), "2018-01-11T10:05:13.084+08:00");
    assert_eq!(dt.to_rfc3339_opts(Micros, false), "2018-01-11T10:05:13.084660+08:00");
    assert_eq!(dt.to_rfc3339_opts(Nanos, false), "2018-01-11T10:05:13.084660000+08:00");
    assert_eq!(dt.to_rfc3339_opts(AutoSi, false), "2018-01-11T10:05:13.084660+08:00");

    let ut = DateTime::<Utc>::from_utc(dt.naive_utc(), Utc);
    assert_eq!(ut.to_rfc3339_opts(Secs, false), "2018-01-11T02:05:13+00:00");
    assert_eq!(ut.to_rfc3339_opts(Secs, true), "2018-01-11T02:05:13Z");
    assert_eq!(ut.to_rfc3339_opts(Millis, false), "2018-01-11T02:05:13.084+00:00");
    assert_eq!(ut.to_rfc3339_opts(Millis, true), "2018-01-11T02:05:13.084Z");
    assert_eq!(ut.to_rfc3339_opts(Micros, true), "2018-01-11T02:05:13.084660Z");
    assert_eq!(ut.to_rfc3339_opts(Nanos, true), "2018-01-11T02:05:13.084660000Z");
    assert_eq!(ut.to_rfc3339_opts(AutoSi, true), "2018-01-11T02:05:13.084660Z");
}

#[test]
#[should_panic]
fn test_rfc3339_opts_nonexhaustive() {
    use crate::SecondsFormat;
    let dt = Utc.ymd(1999, 10, 9).and_hms(1, 2, 3);
    dt.to_rfc3339_opts(SecondsFormat::__NonExhaustive, true);
}

#[test]
fn test_datetime_from_str() {
    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::east(0).ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert_eq!(
        "2015-02-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(Utc.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert_eq!(
        "2015-02-18T23:16:9.15 UTC".parse::<DateTime<Utc>>(),
        Ok(Utc.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert_eq!(
        "2015-02-18T23:16:9.15UTC".parse::<DateTime<Utc>>(),
        Ok(Utc.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::east(0).ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<FixedOffset>>(),
        Ok(FixedOffset::west(10 * 3600).ymd(2015, 2, 18).and_hms_milli(13, 16, 9, 150))
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<FixedOffset>>().is_err());

    assert_eq!(
        "2015-2-18T23:16:9.15Z".parse::<DateTime<Utc>>(),
        Ok(Utc.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert_eq!(
        "2015-2-18T13:16:9.15-10:00".parse::<DateTime<Utc>>(),
        Ok(Utc.ymd(2015, 2, 18).and_hms_milli(23, 16, 9, 150))
    );
    assert!("2015-2-18T23:16:9.15".parse::<DateTime<Utc>>().is_err());

    // no test for `DateTime<Local>`, we cannot verify that much.
}

#[test]
fn test_datetime_parse_from_str() {
    let ymdhms = |y, m, d, h, n, s, off| FixedOffset::east(off).ymd(y, m, d).and_hms(h, n, s);
    assert_eq!(
        DateTime::parse_from_str("2014-5-7T12:34:56+09:30", "%Y-%m-%dT%H:%M:%S%z"),
        Ok(ymdhms(2014, 5, 7, 12, 34, 56, 570 * 60))
    ); // ignore offset
    assert!(DateTime::parse_from_str("20140507000000", "%Y%m%d%H%M%S").is_err()); // no offset
    assert!(DateTime::parse_from_str("Fri, 09 Aug 2013 23:54:35 GMT", "%a, %d %b %Y %H:%M:%S GMT")
        .is_err());
    assert_eq!(
        Utc.datetime_from_str("Fri, 09 Aug 2013 23:54:35 GMT", "%a, %d %b %Y %H:%M:%S GMT"),
        Ok(Utc.ymd(2013, 8, 9).and_hms(23, 54, 35))
    );
}

#[test]
fn test_to_string_round_trip() {
    let dt = Utc.ymd(2000, 1, 1).and_hms(0, 0, 0);
    let _dt: DateTime<Utc> = dt.to_string().parse().unwrap();

    let ndt_fixed = dt.with_timezone(&FixedOffset::east(3600));
    let _dt: DateTime<FixedOffset> = ndt_fixed.to_string().parse().unwrap();

    let ndt_fixed = dt.with_timezone(&FixedOffset::east(0));
    let _dt: DateTime<FixedOffset> = ndt_fixed.to_string().parse().unwrap();
}

#[test]
#[cfg(feature = "clock")]
fn test_to_string_round_trip_with_local() {
    let ndt = Local::now();
    let _dt: DateTime<FixedOffset> = ndt.to_string().parse().unwrap();
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_format_with_local() {
    // if we are not around the year boundary, local and UTC date should have the same year
    let dt = Local::now().with_month(5).unwrap();
    assert_eq!(dt.format("%Y").to_string(), dt.with_timezone(&Utc).format("%Y").to_string());
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_is_copy() {
    // UTC is known to be `Copy`.
    let a = Utc::now();
    let b = a;
    assert_eq!(a, b);
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_is_send() {
    use std::thread;

    // UTC is known to be `Send`.
    let a = Utc::now();
    thread::spawn(move || {
        let _ = a;
    })
    .join()
    .unwrap();
}

#[test]
fn test_subsecond_part() {
    let datetime = Utc.ymd(2014, 7, 8).and_hms_nano(9, 10, 11, 1234567);

    assert_eq!(1, datetime.timestamp_subsec_millis());
    assert_eq!(1234, datetime.timestamp_subsec_micros());
    assert_eq!(1234567, datetime.timestamp_subsec_nanos());
}

#[test]
#[cfg(not(target_os = "windows"))]
fn test_from_system_time() {
    use std::time::Duration;

    let epoch = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0);
    let nanos = 999_999_999;

    // SystemTime -> DateTime<Utc>
    assert_eq!(DateTime::<Utc>::from(UNIX_EPOCH), epoch);
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH + Duration::new(999_999_999, nanos)),
        Utc.ymd(2001, 9, 9).and_hms_nano(1, 46, 39, nanos)
    );
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH - Duration::new(999_999_999, nanos)),
        Utc.ymd(1938, 4, 24).and_hms_nano(22, 13, 20, 1)
    );

    // DateTime<Utc> -> SystemTime
    assert_eq!(SystemTime::from(epoch), UNIX_EPOCH);
    assert_eq!(
        SystemTime::from(Utc.ymd(2001, 9, 9).and_hms_nano(1, 46, 39, nanos)),
        UNIX_EPOCH + Duration::new(999_999_999, nanos)
    );
    assert_eq!(
        SystemTime::from(Utc.ymd(1938, 4, 24).and_hms_nano(22, 13, 20, 1)),
        UNIX_EPOCH - Duration::new(999_999_999, 999_999_999)
    );

    // DateTime<any tz> -> SystemTime (via `with_timezone`)
    #[cfg(feature = "clock")]
    {
        assert_eq!(SystemTime::from(epoch.with_timezone(&Local)), UNIX_EPOCH);
    }
    assert_eq!(SystemTime::from(epoch.with_timezone(&FixedOffset::east(32400))), UNIX_EPOCH);
    assert_eq!(SystemTime::from(epoch.with_timezone(&FixedOffset::west(28800))), UNIX_EPOCH);
}

#[test]
#[cfg(target_os = "windows")]
fn test_from_system_time() {
    use std::time::Duration;

    let nanos = 999_999_000;

    let epoch = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0);

    // SystemTime -> DateTime<Utc>
    assert_eq!(DateTime::<Utc>::from(UNIX_EPOCH), epoch);
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH + Duration::new(999_999_999, nanos)),
        Utc.ymd(2001, 9, 9).and_hms_nano(1, 46, 39, nanos)
    );
    assert_eq!(
        DateTime::<Utc>::from(UNIX_EPOCH - Duration::new(999_999_999, nanos)),
        Utc.ymd(1938, 4, 24).and_hms_nano(22, 13, 20, 1_000)
    );

    // DateTime<Utc> -> SystemTime
    assert_eq!(SystemTime::from(epoch), UNIX_EPOCH);
    assert_eq!(
        SystemTime::from(Utc.ymd(2001, 9, 9).and_hms_nano(1, 46, 39, nanos)),
        UNIX_EPOCH + Duration::new(999_999_999, nanos)
    );
    assert_eq!(
        SystemTime::from(Utc.ymd(1938, 4, 24).and_hms_nano(22, 13, 20, 1_000)),
        UNIX_EPOCH - Duration::new(999_999_999, nanos)
    );

    // DateTime<any tz> -> SystemTime (via `with_timezone`)
    #[cfg(feature = "clock")]
    {
        assert_eq!(SystemTime::from(epoch.with_timezone(&Local)), UNIX_EPOCH);
    }
    assert_eq!(SystemTime::from(epoch.with_timezone(&FixedOffset::east(32400))), UNIX_EPOCH);
    assert_eq!(SystemTime::from(epoch.with_timezone(&FixedOffset::west(28800))), UNIX_EPOCH);
}

#[test]
fn test_datetime_format_alignment() {
    let datetime = Utc.ymd(2007, 1, 2);

    // Item::Literal
    let percent = datetime.format("%%");
    assert_eq!("  %", format!("{:>3}", percent));
    assert_eq!("%  ", format!("{:<3}", percent));
    assert_eq!(" % ", format!("{:^3}", percent));

    // Item::Numeric
    let year = datetime.format("%Y");
    assert_eq!("  2007", format!("{:>6}", year));
    assert_eq!("2007  ", format!("{:<6}", year));
    assert_eq!(" 2007 ", format!("{:^6}", year));

    // Item::Fixed
    let tz = datetime.format("%Z");
    assert_eq!("  UTC", format!("{:>5}", tz));
    assert_eq!("UTC  ", format!("{:<5}", tz));
    assert_eq!(" UTC ", format!("{:^5}", tz));

    // [Item::Numeric, Item::Space, Item::Literal, Item::Space, Item::Numeric]
    let ymd = datetime.format("%Y %B %d");
    let ymd_formatted = "2007 January 02";
    assert_eq!(format!("  {}", ymd_formatted), format!("{:>17}", ymd));
    assert_eq!(format!("{}  ", ymd_formatted), format!("{:<17}", ymd));
    assert_eq!(format!(" {} ", ymd_formatted), format!("{:^17}", ymd));
}

#[test]
fn test_datetime_from_local() {
    // 2000-01-12T02:00:00Z
    let naivedatetime_utc = NaiveDate::from_ymd(2000, 1, 12).and_hms(2, 0, 0);
    let datetime_utc = DateTime::<Utc>::from_utc(naivedatetime_utc, Utc);

    // 2000-01-12T10:00:00+8:00:00
    let timezone_east = FixedOffset::east(8 * 60 * 60);
    let naivedatetime_east = NaiveDate::from_ymd(2000, 1, 12).and_hms(10, 0, 0);
    let datetime_east = DateTime::<FixedOffset>::from_local(naivedatetime_east, timezone_east);

    // 2000-01-11T19:00:00-7:00:00
    let timezone_west = FixedOffset::west(7 * 60 * 60);
    let naivedatetime_west = NaiveDate::from_ymd(2000, 1, 11).and_hms(19, 0, 0);
    let datetime_west = DateTime::<FixedOffset>::from_local(naivedatetime_west, timezone_west);

    assert_eq!(datetime_east, datetime_utc.with_timezone(&timezone_east));
    assert_eq!(datetime_west, datetime_utc.with_timezone(&timezone_west));
}

#[test]
#[cfg(feature = "clock")]
fn test_years_elapsed() {
    const WEEKS_PER_YEAR: f32 = 52.1775;

    // This is always at least one year because 1 year = 52.1775 weeks.
    let one_year_ago = Utc::today() - Duration::weeks((WEEKS_PER_YEAR * 1.5).ceil() as i64);
    // A bit more than 2 years.
    let two_year_ago = Utc::today() - Duration::weeks((WEEKS_PER_YEAR * 2.5).ceil() as i64);

    assert_eq!(Utc::today().years_since(one_year_ago), Some(1));
    assert_eq!(Utc::today().years_since(two_year_ago), Some(2));

    // If the given DateTime is later than now, the function will always return 0.
    let future = Utc::today() + Duration::weeks(12);
    assert_eq!(Utc::today().years_since(future), None);
}

#[test]
fn test_datetime_add_assign() {
    let naivedatetime = NaiveDate::from_ymd(2000, 1, 1).and_hms(0, 0, 0);
    let datetime = DateTime::<Utc>::from_utc(naivedatetime, Utc);
    let mut datetime_add = datetime;

    datetime_add += Duration::seconds(60);
    assert_eq!(datetime_add, datetime + Duration::seconds(60));

    let timezone = FixedOffset::east(60 * 60);
    let datetime = datetime.with_timezone(&timezone);
    let datetime_add = datetime_add.with_timezone(&timezone);

    assert_eq!(datetime_add, datetime + Duration::seconds(60));

    let timezone = FixedOffset::west(2 * 60 * 60);
    let datetime = datetime.with_timezone(&timezone);
    let datetime_add = datetime_add.with_timezone(&timezone);

    assert_eq!(datetime_add, datetime + Duration::seconds(60));
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_add_assign_local() {
    let naivedatetime = NaiveDate::from_ymd(2022, 1, 1).and_hms(0, 0, 0);

    let datetime = Local.from_utc_datetime(&naivedatetime);
    let mut datetime_add = Local.from_utc_datetime(&naivedatetime);

    // ensure we cross a DST transition
    for i in 1..=365 {
        datetime_add += Duration::days(1);
        assert_eq!(datetime_add, datetime + Duration::days(i))
    }
}

#[test]
fn test_datetime_sub_assign() {
    let naivedatetime = NaiveDate::from_ymd(2000, 1, 1).and_hms(12, 0, 0);
    let datetime = DateTime::<Utc>::from_utc(naivedatetime, Utc);
    let mut datetime_sub = datetime;

    datetime_sub -= Duration::minutes(90);
    assert_eq!(datetime_sub, datetime - Duration::minutes(90));

    let timezone = FixedOffset::east(60 * 60);
    let datetime = datetime.with_timezone(&timezone);
    let datetime_sub = datetime_sub.with_timezone(&timezone);

    assert_eq!(datetime_sub, datetime - Duration::minutes(90));

    let timezone = FixedOffset::west(2 * 60 * 60);
    let datetime = datetime.with_timezone(&timezone);
    let datetime_sub = datetime_sub.with_timezone(&timezone);

    assert_eq!(datetime_sub, datetime - Duration::minutes(90));
}

#[test]
#[cfg(feature = "clock")]
fn test_datetime_sub_assign_local() {
    let naivedatetime = NaiveDate::from_ymd(2022, 1, 1).and_hms(0, 0, 0);

    let datetime = Local.from_utc_datetime(&naivedatetime);
    let mut datetime_sub = Local.from_utc_datetime(&naivedatetime);

    // ensure we cross a DST transition
    for i in 1..=365 {
        datetime_sub -= Duration::days(1);
        assert_eq!(datetime_sub, datetime - Duration::days(i))
    }
}
