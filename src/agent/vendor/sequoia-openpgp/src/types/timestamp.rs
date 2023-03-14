use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::fmt;
use std::time::{SystemTime, Duration as SystemDuration, UNIX_EPOCH};
use std::u32;

#[cfg(test)]
use quickcheck::{Arbitrary, Gen};

use crate::{
    Error,
    Result,
};

/// A timestamp representable by OpenPGP.
///
/// OpenPGP timestamps are represented as `u32` containing the number of seconds
/// elapsed since midnight, 1 January 1970 UTC ([Section 3.5 of RFC 4880]).
///
/// They cannot express dates further than 7th February of 2106 or earlier than
/// the [UNIX epoch]. Unlike Unix's `time_t`, OpenPGP's timestamp is unsigned so
/// it rollsover in 2106, not 2038.
///
/// # Examples
///
/// Signature creation time is internally stored as a `Timestamp`:
///
/// Note that this example retrieves raw packet value.
/// Use [`SubpacketAreas::signature_creation_time`] to get the signature creation time.
///
/// [`SubpacketAreas::signature_creation_time`]: crate::packet::signature::subpacket::SubpacketAreas::signature_creation_time()
///
/// ```
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use std::convert::From;
/// use std::time::SystemTime;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::packet::signature::subpacket::{SubpacketTag, SubpacketValue};
///
/// # fn main() -> Result<()> {
/// let (cert, _) =
///     CertBuilder::general_purpose(None, Some("alice@example.org"))
///     .generate()?;
///
/// let subkey = cert.keys().subkeys().next().unwrap();
/// let packets = subkey.bundle().self_signatures()[0].hashed_area();
///
/// match packets.subpacket(SubpacketTag::SignatureCreationTime).unwrap().value() {
///     SubpacketValue::SignatureCreationTime(ts) => assert!(u32::from(*ts) > 0),
///     v => panic!("Unexpected subpacket: {:?}", v),
/// }
///
/// let p = &StandardPolicy::new();
/// let now = SystemTime::now();
/// assert!(subkey.binding_signature(p, now)?.signature_creation_time().is_some());
/// # Ok(()) }
/// ```
///
/// [Section 3.5 of RFC 4880]: https://tools.ietf.org/html/rfc4880#section-3.5
/// [UNIX epoch]: https://en.wikipedia.org/wiki/Unix_time
/// [`Timestamp::round_down`]: crate::types::Timestamp::round_down()
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u32);
assert_send_and_sync!(Timestamp);

impl From<Timestamp> for u32 {
    fn from(t: Timestamp) -> Self {
        t.0
    }
}

impl From<u32> for Timestamp {
    fn from(t: u32) -> Self {
        Timestamp(t)
    }
}

impl TryFrom<SystemTime> for Timestamp {
    type Error = anyhow::Error;

    fn try_from(t: SystemTime) -> Result<Self> {
        match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) if d.as_secs() <= std::u32::MAX as u64 =>
                Ok(Timestamp(d.as_secs() as u32)),
            _ => Err(Error::InvalidArgument(
                format!("Time exceeds u32 epoch: {:?}", t))
                     .into()),
        }
    }
}

/// SystemTime's underlying datatype may be only `i32`, e.g. on 32bit Unix.
/// As OpenPGP's timestamp datatype is `u32`, there are timestamps (`i32::MAX + 1`
/// to `u32::MAX`) which are not representable on such systems.
///
/// In this case, the result is clamped to `i32::MAX`.
impl From<Timestamp> for SystemTime {
    fn from(t: Timestamp) -> Self {
        UNIX_EPOCH.checked_add(SystemDuration::new(t.0 as u64, 0))
            .unwrap_or_else(|| UNIX_EPOCH + SystemDuration::new(i32::MAX as u64, 0))
    }
}

impl From<Timestamp> for Option<SystemTime> {
    fn from(t: Timestamp) -> Self {
        Some(t.into())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", crate::fmt::time(&SystemTime::from(*self)))
    }
}

impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Timestamp {
    /// Returns the current time.
    pub fn now() -> Timestamp {
        crate::now().try_into()
            .expect("representable for the next hundred years")
    }

    /// Adds a duration to this timestamp.
    ///
    /// Returns `None` if the resulting timestamp is not
    /// representable.
    pub fn checked_add(&self, d: Duration) -> Option<Timestamp> {
        self.0.checked_add(d.0).map(Self)
    }

    /// Subtracts a duration from this timestamp.
    ///
    /// Returns `None` if the resulting timestamp is not
    /// representable.
    pub fn checked_sub(&self, d: Duration) -> Option<Timestamp> {
        self.0.checked_sub(d.0).map(Self)
    }

    /// Rounds down to the given level of precision.
    ///
    /// This can be used to reduce the metadata leak resulting from
    /// time stamps.  For example, a group of people attending a key
    /// signing event could be identified by comparing the time stamps
    /// of resulting certifications.  By rounding the creation time of
    /// these signatures down, all of them, and others, fall into the
    /// same bucket.
    ///
    /// The given level `p` determines the resulting resolution of
    /// `2^p` seconds.  The default is `21`, which results in a
    /// resolution of 24 days, or roughly a month.  `p` must be lower
    /// than 32.
    ///
    /// The lower limit `floor` represents the earliest time the timestamp will be
    /// rounded down to.
    ///
    /// See also [`Duration::round_up`](Duration::round_up()).
    ///
    /// # Important note
    ///
    /// If we create a signature, it is important that the signature's
    /// creation time does not predate the signing keys creation time,
    /// or otherwise violate the key's validity constraints.
    /// This can be achieved by using the `floor` parameter.
    ///
    /// To ensure validity, use this function to round the time down,
    /// using the latest known relevant timestamp as a floor.
    /// Then, lookup all keys and other objects like userids using this
    /// timestamp, and on success create the signature:
    ///
    /// ```rust
    /// # use sequoia_openpgp::{*, packet::prelude::*, types::*, cert::*};
    /// use sequoia_openpgp::policy::StandardPolicy;
    ///
    /// # fn main() -> Result<()> {
    /// let policy = &StandardPolicy::new();
    ///
    /// // Let's fix a time.
    /// let now = Timestamp::from(1583436160);
    ///
    /// let cert_creation_alice = now.checked_sub(Duration::weeks(2)?).unwrap();
    /// let cert_creation_bob = now.checked_sub(Duration::weeks(1)?).unwrap();
    ///
    /// // Generate a Cert for Alice.
    /// let (alice, _) = CertBuilder::new()
    ///     .set_creation_time(cert_creation_alice)
    ///     .set_primary_key_flags(KeyFlags::empty().set_certification())
    ///     .add_userid("alice@example.org")
    ///     .generate()?;
    ///
    /// // Generate a Cert for Bob.
    /// let (bob, _) = CertBuilder::new()
    ///     .set_creation_time(cert_creation_bob)
    ///     .set_primary_key_flags(KeyFlags::empty().set_certification())
    ///     .add_userid("bob@example.org")
    ///     .generate()?;
    ///
    /// let sign_with_p = |p| -> Result<Signature> {
    ///     // Round `now` down, then use `t` for all lookups.
    ///     // Use the creation time of Bob's Cert as lower bound for rounding.
    ///     let t: std::time::SystemTime = now.round_down(p, cert_creation_bob)?.into();
    ///
    ///     // First, get the certification key.
    ///     let mut keypair =
    ///         alice.keys().with_policy(policy, t).secret().for_certification()
    ///         .nth(0).ok_or_else(|| anyhow::anyhow!("no valid key at"))?
    ///         .key().clone().into_keypair()?;
    ///
    ///     // Then, lookup the binding between `bob@example.org` and
    ///     // `bob` at `t`.
    ///     let ca = bob.userids().with_policy(policy, t)
    ///         .filter(|ca| ca.userid().value() == b"bob@example.org")
    ///         .nth(0).ok_or_else(|| anyhow::anyhow!("no valid userid"))?;
    ///
    ///     // Finally, Alice certifies the binding between
    ///     // `bob@example.org` and `bob` at `t`.
    ///     ca.userid().certify(&mut keypair, &bob,
    ///                         SignatureType::PositiveCertification, None, t)
    /// };
    ///
    /// assert!(sign_with_p(21).is_ok());
    /// assert!(sign_with_p(22).is_ok());  // Rounded to bob's cert's creation time.
    /// assert!(sign_with_p(32).is_err()); // Invalid precision
    /// # Ok(()) }
    /// ```
    pub fn round_down<P, F>(&self, precision: P, floor: F) -> Result<Timestamp>
        where P: Into<Option<u8>>,
              F: Into<Option<SystemTime>>
    {
        let p = precision.into().unwrap_or(21) as u32;
        if p < 32 {
            let rounded = Self(self.0 & !((1 << p) - 1));
            match floor.into() {
                Some(floor) => {
                    Ok(cmp::max(rounded, floor.try_into()?))
                }
                None => { Ok(rounded) }
            }
        } else {
            Err(Error::InvalidArgument(
                format!("Invalid precision {}", p)).into())
        }
    }
}

#[cfg(test)]
impl Arbitrary for Timestamp {
    fn arbitrary(g: &mut Gen) -> Self {
        Timestamp(u32::arbitrary(g))
    }
}

/// A duration representable by OpenPGP.
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// # use openpgp::Result;
/// use openpgp::cert::prelude::*;
/// use openpgp::policy::StandardPolicy;
/// use openpgp::packet::signature::subpacket::{SubpacketTag, SubpacketValue};
/// use openpgp::types::{Timestamp, Duration};
///
/// # fn main() -> Result<()> {
/// let p = &StandardPolicy::new();
///
/// let now = Timestamp::now();
/// let validity_period = Duration::days(365)?;
///
/// let (cert,_) = CertBuilder::new()
///     .set_creation_time(now)
///     .set_validity_period(validity_period)
///     .generate()?;
///
/// let vc = cert.with_policy(p, now)?;
/// assert!(vc.alive().is_ok());
/// # Ok(()) }
/// ```
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration(u32);
assert_send_and_sync!(Duration);

impl From<Duration> for u32 {
    fn from(d: Duration) -> Self {
        d.0
    }
}

impl From<u32> for Duration {
    fn from(d: u32) -> Self {
        Duration(d)
    }
}

impl TryFrom<SystemDuration> for Duration {
    type Error = anyhow::Error;

    fn try_from(d: SystemDuration) -> Result<Self> {
        if d.as_secs() <= std::u32::MAX as u64 {
            Ok(Duration(d.as_secs() as u32))
        } else {
            Err(Error::InvalidArgument(
                format!("Duration exceeds u32: {:?}", d))
                     .into())
        }
    }
}

impl From<Duration> for SystemDuration {
    fn from(d: Duration) -> Self {
        SystemDuration::new(d.0 as u64, 0)
    }
}

impl From<Duration> for Option<SystemDuration> {
    fn from(d: Duration) -> Self {
        Some(d.into())
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", SystemDuration::from(*self))
    }
}

impl Duration {
    /// Returns a `Duration` with the given number of seconds.
    pub const fn seconds(n: u32) -> Duration {
        Self(n)
    }

    /// Returns a `Duration` with the given number of minutes, if
    /// representable.
    pub fn minutes(n: u32) -> Result<Duration> {
        match 60u32.checked_mul(n) {
            Some(val) => Ok(Self::seconds(val)),
            None => {
                Err(Error::InvalidArgument(format!(
                    "Not representable: {} minutes in seconds exceeds u32",
                    n
                )).into())
            }
        }
    }

    /// Returns a `Duration` with the given number of hours, if
    /// representable.
    pub fn hours(n: u32) -> Result<Duration> {
        match 60u32.checked_mul(n) {
            Some(val) => Self::minutes(val),
            None => {
                Err(Error::InvalidArgument(format!(
                    "Not representable: {} hours in seconds exceeds u32",
                    n
                )).into())
            }
        }
    }

    /// Returns a `Duration` with the given number of days, if
    /// representable.
    pub fn days(n: u32) -> Result<Duration> {
        match 24u32.checked_mul(n) {
            Some(val) => Self::hours(val),
            None => {
                Err(Error::InvalidArgument(format!(
                    "Not representable: {} days in seconds exceeds u32",
                    n
                )).into())
            }
        }
    }

    /// Returns a `Duration` with the given number of weeks, if
    /// representable.
    pub fn weeks(n: u32) -> Result<Duration> {
        match 7u32.checked_mul(n) {
            Some(val) => Self::days(val),
            None => {
                Err(Error::InvalidArgument(format!(
                    "Not representable: {} weeks in seconds exceeds u32",
                    n
                )).into())
            }
        }
    }

    /// Returns a `Duration` with the given number of years, if
    /// representable.
    ///
    /// This function assumes that there are 365.2425 [days in a
    /// year], the average number of days in a year in the Gregorian
    /// calendar.
    ///
    ///   [days in a year]: https://en.wikipedia.org/wiki/Year
    pub fn years(n: u32) -> Result<Duration> {
        let s = (365.2425 * n as f64).trunc();
        if s > u32::MAX as f64 {
            Err(Error::InvalidArgument(
                format!("Not representable: {} years in seconds exceeds u32",
                        n))
                .into())
        } else {
            Ok((s as u32).into())
        }
    }

    /// Returns the duration as seconds.
    pub fn as_secs(self) -> u64 {
        self.0 as u64
    }

    /// Rounds up to the given level of precision.
    ///
    /// If [`Timestamp::round_down`] is used to round the creation
    /// timestamp of a key or signature down, then this function may
    /// be used to round the corresponding expiration time up.  This
    /// ensures validity during the originally intended lifetime,
    /// while avoiding the metadata leak associated with preserving
    /// the originally intended expiration time.
    ///
    ///   [`Timestamp::round_down`]: Timestamp::round_down()
    ///
    /// The given level `p` determines the resulting resolution of
    /// `2^p` seconds.  The default is `21`, which results in a
    /// resolution of 24 days, or roughly a month.  `p` must be lower
    /// than 32.
    ///
    /// The upper limit `ceil` represents the maximum time to round up to.
    pub fn round_up<P, C>(&self, precision: P, ceil: C) -> Result<Duration>
        where P: Into<Option<u8>>,
              C: Into<Option<SystemDuration>>
    {
        let p = precision.into().unwrap_or(21) as u32;
        if p < 32 {
            if let Some(sum) = self.0.checked_add((1 << p) - 1) {
                let rounded = Self(sum & !((1 << p) - 1));
                match ceil.into() {
                    Some(ceil) => {
                        Ok(cmp::min(rounded, ceil.try_into()?))
                    },
                    None => Ok(rounded)
                }
            } else {
                Ok(Self(std::u32::MAX))
            }
        } else {
            Err(Error::InvalidArgument(
                format!("Invalid precision {}", p)).into())
        }
    }
}

#[allow(unused)]
impl Timestamp {
    pub(crate) const UNIX_EPOCH : Timestamp = Timestamp(0);
    pub(crate) const MAX : Timestamp = Timestamp(u32::MAX);

    pub(crate) const Y1970 : Timestamp = Timestamp(0);
    // for y in $(seq 1970 2106); do echo "    pub(crate) const Y${y}M2 : Timestamp = Timestamp($(date -u --date="Feb. 1, $y" '+%s'));"; done
    pub(crate) const Y1970M2 : Timestamp = Timestamp(2678400);
    pub(crate) const Y1971M2 : Timestamp = Timestamp(34214400);
    pub(crate) const Y1972M2 : Timestamp = Timestamp(65750400);
    pub(crate) const Y1973M2 : Timestamp = Timestamp(97372800);
    pub(crate) const Y1974M2 : Timestamp = Timestamp(128908800);
    pub(crate) const Y1975M2 : Timestamp = Timestamp(160444800);
    pub(crate) const Y1976M2 : Timestamp = Timestamp(191980800);
    pub(crate) const Y1977M2 : Timestamp = Timestamp(223603200);
    pub(crate) const Y1978M2 : Timestamp = Timestamp(255139200);
    pub(crate) const Y1979M2 : Timestamp = Timestamp(286675200);
    pub(crate) const Y1980M2 : Timestamp = Timestamp(318211200);
    pub(crate) const Y1981M2 : Timestamp = Timestamp(349833600);
    pub(crate) const Y1982M2 : Timestamp = Timestamp(381369600);
    pub(crate) const Y1983M2 : Timestamp = Timestamp(412905600);
    pub(crate) const Y1984M2 : Timestamp = Timestamp(444441600);
    pub(crate) const Y1985M2 : Timestamp = Timestamp(476064000);
    pub(crate) const Y1986M2 : Timestamp = Timestamp(507600000);
    pub(crate) const Y1987M2 : Timestamp = Timestamp(539136000);
    pub(crate) const Y1988M2 : Timestamp = Timestamp(570672000);
    pub(crate) const Y1989M2 : Timestamp = Timestamp(602294400);
    pub(crate) const Y1990M2 : Timestamp = Timestamp(633830400);
    pub(crate) const Y1991M2 : Timestamp = Timestamp(665366400);
    pub(crate) const Y1992M2 : Timestamp = Timestamp(696902400);
    pub(crate) const Y1993M2 : Timestamp = Timestamp(728524800);
    pub(crate) const Y1994M2 : Timestamp = Timestamp(760060800);
    pub(crate) const Y1995M2 : Timestamp = Timestamp(791596800);
    pub(crate) const Y1996M2 : Timestamp = Timestamp(823132800);
    pub(crate) const Y1997M2 : Timestamp = Timestamp(854755200);
    pub(crate) const Y1998M2 : Timestamp = Timestamp(886291200);
    pub(crate) const Y1999M2 : Timestamp = Timestamp(917827200);
    pub(crate) const Y2000M2 : Timestamp = Timestamp(949363200);
    pub(crate) const Y2001M2 : Timestamp = Timestamp(980985600);
    pub(crate) const Y2002M2 : Timestamp = Timestamp(1012521600);
    pub(crate) const Y2003M2 : Timestamp = Timestamp(1044057600);
    pub(crate) const Y2004M2 : Timestamp = Timestamp(1075593600);
    pub(crate) const Y2005M2 : Timestamp = Timestamp(1107216000);
    pub(crate) const Y2006M2 : Timestamp = Timestamp(1138752000);
    pub(crate) const Y2007M2 : Timestamp = Timestamp(1170288000);
    pub(crate) const Y2008M2 : Timestamp = Timestamp(1201824000);
    pub(crate) const Y2009M2 : Timestamp = Timestamp(1233446400);
    pub(crate) const Y2010M2 : Timestamp = Timestamp(1264982400);
    pub(crate) const Y2011M2 : Timestamp = Timestamp(1296518400);
    pub(crate) const Y2012M2 : Timestamp = Timestamp(1328054400);
    pub(crate) const Y2013M2 : Timestamp = Timestamp(1359676800);
    pub(crate) const Y2014M2 : Timestamp = Timestamp(1391212800);
    pub(crate) const Y2015M2 : Timestamp = Timestamp(1422748800);
    pub(crate) const Y2016M2 : Timestamp = Timestamp(1454284800);
    pub(crate) const Y2017M2 : Timestamp = Timestamp(1485907200);
    pub(crate) const Y2018M2 : Timestamp = Timestamp(1517443200);
    pub(crate) const Y2019M2 : Timestamp = Timestamp(1548979200);
    pub(crate) const Y2020M2 : Timestamp = Timestamp(1580515200);
    pub(crate) const Y2021M2 : Timestamp = Timestamp(1612137600);
    pub(crate) const Y2022M2 : Timestamp = Timestamp(1643673600);
    pub(crate) const Y2023M2 : Timestamp = Timestamp(1675209600);
    pub(crate) const Y2024M2 : Timestamp = Timestamp(1706745600);
    pub(crate) const Y2025M2 : Timestamp = Timestamp(1738368000);
    pub(crate) const Y2026M2 : Timestamp = Timestamp(1769904000);
    pub(crate) const Y2027M2 : Timestamp = Timestamp(1801440000);
    pub(crate) const Y2028M2 : Timestamp = Timestamp(1832976000);
    pub(crate) const Y2029M2 : Timestamp = Timestamp(1864598400);
    pub(crate) const Y2030M2 : Timestamp = Timestamp(1896134400);
    pub(crate) const Y2031M2 : Timestamp = Timestamp(1927670400);
    pub(crate) const Y2032M2 : Timestamp = Timestamp(1959206400);
    pub(crate) const Y2033M2 : Timestamp = Timestamp(1990828800);
    pub(crate) const Y2034M2 : Timestamp = Timestamp(2022364800);
    pub(crate) const Y2035M2 : Timestamp = Timestamp(2053900800);
    pub(crate) const Y2036M2 : Timestamp = Timestamp(2085436800);
    pub(crate) const Y2037M2 : Timestamp = Timestamp(2117059200);
    pub(crate) const Y2038M2 : Timestamp = Timestamp(2148595200);
    pub(crate) const Y2039M2 : Timestamp = Timestamp(2180131200);
    pub(crate) const Y2040M2 : Timestamp = Timestamp(2211667200);
    pub(crate) const Y2041M2 : Timestamp = Timestamp(2243289600);
    pub(crate) const Y2042M2 : Timestamp = Timestamp(2274825600);
    pub(crate) const Y2043M2 : Timestamp = Timestamp(2306361600);
    pub(crate) const Y2044M2 : Timestamp = Timestamp(2337897600);
    pub(crate) const Y2045M2 : Timestamp = Timestamp(2369520000);
    pub(crate) const Y2046M2 : Timestamp = Timestamp(2401056000);
    pub(crate) const Y2047M2 : Timestamp = Timestamp(2432592000);
    pub(crate) const Y2048M2 : Timestamp = Timestamp(2464128000);
    pub(crate) const Y2049M2 : Timestamp = Timestamp(2495750400);
    pub(crate) const Y2050M2 : Timestamp = Timestamp(2527286400);
    pub(crate) const Y2051M2 : Timestamp = Timestamp(2558822400);
    pub(crate) const Y2052M2 : Timestamp = Timestamp(2590358400);
    pub(crate) const Y2053M2 : Timestamp = Timestamp(2621980800);
    pub(crate) const Y2054M2 : Timestamp = Timestamp(2653516800);
    pub(crate) const Y2055M2 : Timestamp = Timestamp(2685052800);
    pub(crate) const Y2056M2 : Timestamp = Timestamp(2716588800);
    pub(crate) const Y2057M2 : Timestamp = Timestamp(2748211200);
    pub(crate) const Y2058M2 : Timestamp = Timestamp(2779747200);
    pub(crate) const Y2059M2 : Timestamp = Timestamp(2811283200);
    pub(crate) const Y2060M2 : Timestamp = Timestamp(2842819200);
    pub(crate) const Y2061M2 : Timestamp = Timestamp(2874441600);
    pub(crate) const Y2062M2 : Timestamp = Timestamp(2905977600);
    pub(crate) const Y2063M2 : Timestamp = Timestamp(2937513600);
    pub(crate) const Y2064M2 : Timestamp = Timestamp(2969049600);
    pub(crate) const Y2065M2 : Timestamp = Timestamp(3000672000);
    pub(crate) const Y2066M2 : Timestamp = Timestamp(3032208000);
    pub(crate) const Y2067M2 : Timestamp = Timestamp(3063744000);
    pub(crate) const Y2068M2 : Timestamp = Timestamp(3095280000);
    pub(crate) const Y2069M2 : Timestamp = Timestamp(3126902400);
    pub(crate) const Y2070M2 : Timestamp = Timestamp(3158438400);
    pub(crate) const Y2071M2 : Timestamp = Timestamp(3189974400);
    pub(crate) const Y2072M2 : Timestamp = Timestamp(3221510400);
    pub(crate) const Y2073M2 : Timestamp = Timestamp(3253132800);
    pub(crate) const Y2074M2 : Timestamp = Timestamp(3284668800);
    pub(crate) const Y2075M2 : Timestamp = Timestamp(3316204800);
    pub(crate) const Y2076M2 : Timestamp = Timestamp(3347740800);
    pub(crate) const Y2077M2 : Timestamp = Timestamp(3379363200);
    pub(crate) const Y2078M2 : Timestamp = Timestamp(3410899200);
    pub(crate) const Y2079M2 : Timestamp = Timestamp(3442435200);
    pub(crate) const Y2080M2 : Timestamp = Timestamp(3473971200);
    pub(crate) const Y2081M2 : Timestamp = Timestamp(3505593600);
    pub(crate) const Y2082M2 : Timestamp = Timestamp(3537129600);
    pub(crate) const Y2083M2 : Timestamp = Timestamp(3568665600);
    pub(crate) const Y2084M2 : Timestamp = Timestamp(3600201600);
    pub(crate) const Y2085M2 : Timestamp = Timestamp(3631824000);
    pub(crate) const Y2086M2 : Timestamp = Timestamp(3663360000);
    pub(crate) const Y2087M2 : Timestamp = Timestamp(3694896000);
    pub(crate) const Y2088M2 : Timestamp = Timestamp(3726432000);
    pub(crate) const Y2089M2 : Timestamp = Timestamp(3758054400);
    pub(crate) const Y2090M2 : Timestamp = Timestamp(3789590400);
    pub(crate) const Y2091M2 : Timestamp = Timestamp(3821126400);
    pub(crate) const Y2092M2 : Timestamp = Timestamp(3852662400);
    pub(crate) const Y2093M2 : Timestamp = Timestamp(3884284800);
    pub(crate) const Y2094M2 : Timestamp = Timestamp(3915820800);
    pub(crate) const Y2095M2 : Timestamp = Timestamp(3947356800);
    pub(crate) const Y2096M2 : Timestamp = Timestamp(3978892800);
    pub(crate) const Y2097M2 : Timestamp = Timestamp(4010515200);
    pub(crate) const Y2098M2 : Timestamp = Timestamp(4042051200);
    pub(crate) const Y2099M2 : Timestamp = Timestamp(4073587200);
    pub(crate) const Y2100M2 : Timestamp = Timestamp(4105123200);
    pub(crate) const Y2101M2 : Timestamp = Timestamp(4136659200);
    pub(crate) const Y2102M2 : Timestamp = Timestamp(4168195200);
    pub(crate) const Y2103M2 : Timestamp = Timestamp(4199731200);
    pub(crate) const Y2104M2 : Timestamp = Timestamp(4231267200);
    pub(crate) const Y2105M2 : Timestamp = Timestamp(4262889600);
    pub(crate) const Y2106M2 : Timestamp = Timestamp(4294425600);
}

#[cfg(test)]
impl Arbitrary for Duration {
    fn arbitrary(g: &mut Gen) -> Self {
        Duration(u32::arbitrary(g))
    }
}

/// Normalizes the given SystemTime to the resolution OpenPGP
/// supports.
pub(crate) fn normalize_systemtime(t: SystemTime) -> SystemTime {
    UNIX_EPOCH + SystemDuration::new(
        t.duration_since(UNIX_EPOCH).unwrap().as_secs(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    quickcheck! {
        fn timestamp_round_down(t: Timestamp) -> bool {
            let u = t.round_down(None, None).unwrap();
            assert!(u <= t);
            assert_eq!(u32::from(u) & 0b1_1111_1111_1111_1111_1111, 0);
            assert!(u32::from(t) - u32::from(u) < 2_u32.pow(21));
            true
        }
    }

    #[test]
    fn timestamp_round_down_floor() -> Result<()> {
        let t = Timestamp(1585753307);
        let floor = t.checked_sub(Duration::weeks(1).unwrap()).unwrap();

        let u = t.round_down(21, floor).unwrap();
        assert!(u < t);
        assert!(floor < u);
        assert_eq!(u32::from(u) & 0b1_1111_1111_1111_1111_1111, 0);

        let floor = t.checked_sub(Duration::days(1).unwrap()).unwrap();

        let u = t.round_down(21, floor).unwrap();
        assert_eq!(u, floor);
        Ok(())
    }

    quickcheck! {
        fn duration_round_up(d: Duration) -> bool {
            let u = d.round_up(None, None).unwrap();
            assert!(d <= u);
            assert!(u32::from(u) & 0b1_1111_1111_1111_1111_1111 == 0
                || u32::from(u) == u32::MAX
            );
            assert!(u32::from(u) - u32::from(d) < 2_u32.pow(21));
            true
        }
    }

    #[test]
    fn duration_round_up_ceil() -> Result<()> {
        let d = Duration(123);

        let ceil = Duration(2_u32.pow(23));

        let u = d.round_up(21, ceil)?;
        assert!(d < u);
        assert!(u < ceil);
        assert_eq!(u32::from(u) & 0b1_1111_1111_1111_1111_1111, 0);

        let ceil = Duration::days(1).unwrap();

        let u = d.round_up(21, ceil)?;
        assert!(d < u);
        assert_eq!(u, ceil);

        Ok(())
    }

    // #668
    // Ensure that, on systems where the SystemTime can only represent values
    // up to i32::MAX (generally, 32-bit systems), Timestamps between
    // i32::MAX + 1 and u32::MAX are clamped down to i32::MAX, and values below
    // are not altered.
    #[test]
    fn system_time_32_bit() -> Result<()> {
        let is_system_time_too_small = UNIX_EPOCH
            .checked_add(SystemDuration::new(i32::MAX as u64 + 1, 0))
            .is_none();

        let t1 = Timestamp::from(i32::MAX as u32 - 1);
        let t2 = Timestamp::from(i32::MAX as u32);
        let t3 = Timestamp::from(i32::MAX as u32 + 1);
        let t4 = Timestamp::from(u32::MAX);

        if is_system_time_too_small {
          assert_eq!(SystemTime::from(t1),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64 - 1, 0));

          assert_eq!(SystemTime::from(t2),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64, 0));

          assert_eq!(SystemTime::from(t3),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64, 0));

          assert_eq!(SystemTime::from(t4),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64, 0));
        } else {
          assert_eq!(SystemTime::from(t1),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64 - 1, 0));

          assert_eq!(SystemTime::from(t2),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64, 0));

          assert_eq!(SystemTime::from(t3),
                     UNIX_EPOCH + SystemDuration::new(i32::MAX as u64 + 1, 0));

          assert_eq!(SystemTime::from(t4),
                     UNIX_EPOCH + SystemDuration::new(u32::MAX as u64, 0));
        }
        Ok(())
    }
}
