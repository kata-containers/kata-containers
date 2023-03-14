use crate::client::ServerName;
use crate::key;
use crate::msgs::base::{PayloadU16, PayloadU8};
use crate::msgs::codec::{Codec, Reader};
use crate::msgs::enums::{CipherSuite, ProtocolVersion};
use crate::msgs::handshake::CertificatePayload;
use crate::msgs::handshake::SessionID;
use crate::suites::SupportedCipherSuite;
use crate::ticketer::TimeBase;
#[cfg(feature = "tls12")]
use crate::tls12::Tls12CipherSuite;
use crate::tls13::Tls13CipherSuite;

use std::cmp;
#[cfg(feature = "tls12")]
use std::mem;

// These are the keys and values we store in session storage.

// --- Client types ---
/// Keys for session resumption and tickets.
/// Matching value is a `ClientSessionValue`.
#[derive(Debug)]
pub struct ClientSessionKey {
    kind: &'static [u8],
    name: Vec<u8>,
}

impl Codec for ClientSessionKey {
    fn encode(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(self.kind);
        bytes.extend_from_slice(&self.name);
    }

    // Don't need to read these.
    fn read(_r: &mut Reader) -> Option<Self> {
        None
    }
}

impl ClientSessionKey {
    pub fn session_for_server_name(server_name: &ServerName) -> Self {
        Self {
            kind: b"session",
            name: server_name.encode(),
        }
    }

    pub fn hint_for_server_name(server_name: &ServerName) -> Self {
        Self {
            kind: b"kx-hint",
            name: server_name.encode(),
        }
    }
}

#[derive(Debug)]
pub enum ClientSessionValue {
    Tls13(Tls13ClientSessionValue),
    #[cfg(feature = "tls12")]
    Tls12(Tls12ClientSessionValue),
}

impl ClientSessionValue {
    pub fn read(
        reader: &mut Reader<'_>,
        suite: CipherSuite,
        supported: &[SupportedCipherSuite],
    ) -> Option<Self> {
        match supported
            .iter()
            .find(|s| s.suite() == suite)?
        {
            SupportedCipherSuite::Tls13(inner) => {
                Tls13ClientSessionValue::read(inner, reader).map(ClientSessionValue::Tls13)
            }
            #[cfg(feature = "tls12")]
            SupportedCipherSuite::Tls12(inner) => {
                Tls12ClientSessionValue::read(inner, reader).map(ClientSessionValue::Tls12)
            }
        }
    }

    fn common(&self) -> &ClientSessionCommon {
        match self {
            ClientSessionValue::Tls13(inner) => &inner.common,
            #[cfg(feature = "tls12")]
            ClientSessionValue::Tls12(inner) => &inner.common,
        }
    }
}

impl From<Tls13ClientSessionValue> for ClientSessionValue {
    fn from(v: Tls13ClientSessionValue) -> Self {
        Self::Tls13(v)
    }
}

#[cfg(feature = "tls12")]
impl From<Tls12ClientSessionValue> for ClientSessionValue {
    fn from(v: Tls12ClientSessionValue) -> Self {
        Self::Tls12(v)
    }
}

pub struct Retrieved<T> {
    pub value: T,
    retrieved_at: TimeBase,
}

impl<T> Retrieved<T> {
    pub fn new(value: T, retrieved_at: TimeBase) -> Self {
        Self {
            value,
            retrieved_at,
        }
    }
}

impl Retrieved<&Tls13ClientSessionValue> {
    pub fn obfuscated_ticket_age(&self) -> u32 {
        let age_secs = self
            .retrieved_at
            .as_secs()
            .saturating_sub(self.value.common.epoch);
        let age_millis = age_secs as u32 * 1000;
        age_millis.wrapping_add(self.value.age_add)
    }
}

impl Retrieved<ClientSessionValue> {
    pub fn tls13(&self) -> Option<Retrieved<&Tls13ClientSessionValue>> {
        match &self.value {
            ClientSessionValue::Tls13(value) => Some(Retrieved::new(value, self.retrieved_at)),
            #[cfg(feature = "tls12")]
            ClientSessionValue::Tls12(_) => None,
        }
    }

    pub fn has_expired(&self) -> bool {
        let common = self.value.common();
        common.lifetime_secs != 0
            && common.epoch + u64::from(common.lifetime_secs) < self.retrieved_at.as_secs()
    }
}

impl<T> std::ops::Deref for Retrieved<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[derive(Debug)]
pub struct Tls13ClientSessionValue {
    suite: &'static Tls13CipherSuite,
    age_add: u32,
    max_early_data_size: u32,
    pub common: ClientSessionCommon,
}

impl Tls13ClientSessionValue {
    pub fn new(
        suite: &'static Tls13CipherSuite,
        ticket: Vec<u8>,
        secret: Vec<u8>,
        server_cert_chain: Vec<key::Certificate>,
        time_now: TimeBase,
        lifetime_secs: u32,
        age_add: u32,
        max_early_data_size: u32,
    ) -> Self {
        Self {
            suite,
            age_add,
            max_early_data_size,
            common: ClientSessionCommon::new(
                ticket,
                secret,
                time_now,
                lifetime_secs,
                server_cert_chain,
            ),
        }
    }

    /// [`Codec::read()`] with an extra `suite` argument.
    ///
    /// We decode the `suite` argument separately because it allows us to
    /// decide whether we're decoding an 1.2 or 1.3 session value.
    pub fn read(suite: &'static Tls13CipherSuite, r: &mut Reader) -> Option<Self> {
        Some(Self {
            suite,
            age_add: u32::read(r)?,
            max_early_data_size: u32::read(r)?,
            common: ClientSessionCommon::read(r)?,
        })
    }

    /// Inherent implementation of the [`Codec::get_encoding()`] method.
    ///
    /// (See `read()` for why this is inherent here.)
    pub fn get_encoding(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        self.suite
            .common
            .suite
            .encode(&mut bytes);
        self.age_add.encode(&mut bytes);
        self.max_early_data_size
            .encode(&mut bytes);
        self.common.encode(&mut bytes);
        bytes
    }

    pub fn max_early_data_size(&self) -> u32 {
        self.max_early_data_size
    }

    pub fn suite(&self) -> &'static Tls13CipherSuite {
        self.suite
    }
}

impl std::ops::Deref for Tls13ClientSessionValue {
    type Target = ClientSessionCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

#[cfg(feature = "tls12")]
#[derive(Debug)]
pub struct Tls12ClientSessionValue {
    suite: &'static Tls12CipherSuite,
    pub session_id: SessionID,
    extended_ms: bool,
    pub common: ClientSessionCommon,
}

#[cfg(feature = "tls12")]
impl Tls12ClientSessionValue {
    pub fn new(
        suite: &'static Tls12CipherSuite,
        session_id: SessionID,
        ticket: Vec<u8>,
        master_secret: Vec<u8>,
        server_cert_chain: Vec<key::Certificate>,
        time_now: TimeBase,
        lifetime_secs: u32,
        extended_ms: bool,
    ) -> Self {
        Self {
            suite,
            session_id,
            extended_ms,
            common: ClientSessionCommon::new(
                ticket,
                master_secret,
                time_now,
                lifetime_secs,
                server_cert_chain,
            ),
        }
    }

    /// [`Codec::read()`] with an extra `suite` argument.
    ///
    /// We decode the `suite` argument separately because it allows us to
    /// decide whether we're decoding an 1.2 or 1.3 session value.
    fn read(suite: &'static Tls12CipherSuite, r: &mut Reader) -> Option<Self> {
        Some(Self {
            suite,
            session_id: SessionID::read(r)?,
            extended_ms: u8::read(r)? == 1,
            common: ClientSessionCommon::read(r)?,
        })
    }

    /// Inherent implementation of the [`Codec::get_encoding()`] method.
    ///
    /// (See `read()` for why this is inherent here.)
    pub fn get_encoding(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16);
        self.suite
            .common
            .suite
            .encode(&mut bytes);
        self.session_id.encode(&mut bytes);
        (if self.extended_ms { 1u8 } else { 0u8 }).encode(&mut bytes);
        self.common.encode(&mut bytes);
        bytes
    }

    pub fn take_ticket(&mut self) -> Vec<u8> {
        mem::take(&mut self.common.ticket.0)
    }

    pub fn extended_ms(&self) -> bool {
        self.extended_ms
    }

    pub fn suite(&self) -> &'static Tls12CipherSuite {
        self.suite
    }
}

#[cfg(feature = "tls12")]
impl std::ops::Deref for Tls12ClientSessionValue {
    type Target = ClientSessionCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

#[derive(Debug)]
pub struct ClientSessionCommon {
    ticket: PayloadU16,
    secret: PayloadU8,
    epoch: u64,
    lifetime_secs: u32,
    server_cert_chain: CertificatePayload,
}

impl ClientSessionCommon {
    fn new(
        ticket: Vec<u8>,
        secret: Vec<u8>,
        time_now: TimeBase,
        lifetime_secs: u32,
        server_cert_chain: Vec<key::Certificate>,
    ) -> Self {
        Self {
            ticket: PayloadU16(ticket),
            secret: PayloadU8(secret),
            epoch: time_now.as_secs(),
            lifetime_secs: cmp::min(lifetime_secs, MAX_TICKET_LIFETIME),
            server_cert_chain,
        }
    }

    /// [`Codec::read()`] is inherent here to avoid leaking the [`Codec`]
    /// implementation through [`Deref`] implementations on
    /// [`Tls12ClientSessionValue`] and [`Tls13ClientSessionValue`].
    fn read(r: &mut Reader) -> Option<Self> {
        Some(Self {
            ticket: PayloadU16::read(r)?,
            secret: PayloadU8::read(r)?,
            epoch: u64::read(r)?,
            lifetime_secs: u32::read(r)?,
            server_cert_chain: CertificatePayload::read(r)?,
        })
    }

    /// [`Codec::encode()`] is inherent here to avoid leaking the [`Codec`]
    /// implementation through [`Deref`] implementations on
    /// [`Tls12ClientSessionValue`] and [`Tls13ClientSessionValue`].
    fn encode(&self, bytes: &mut Vec<u8>) {
        self.ticket.encode(bytes);
        self.secret.encode(bytes);
        self.epoch.encode(bytes);
        self.lifetime_secs.encode(bytes);
        self.server_cert_chain.encode(bytes);
    }

    pub fn server_cert_chain(&self) -> &[key::Certificate] {
        self.server_cert_chain.as_ref()
    }

    pub fn secret(&self) -> &[u8] {
        self.secret.0.as_ref()
    }

    pub fn ticket(&self) -> &[u8] {
        self.ticket.0.as_ref()
    }

    /// Test only: wind back epoch by delta seconds.
    pub fn rewind_epoch(&mut self, delta: u32) {
        self.epoch -= delta as u64;
    }
}

static MAX_TICKET_LIFETIME: u32 = 7 * 24 * 60 * 60;

/// This is the maximum allowed skew between server and client clocks, over
/// the maximum ticket lifetime period.  This encompasses TCP retransmission
/// times in case packet loss occurs when the client sends the ClientHello
/// or receives the NewSessionTicket, _and_ actual clock skew over this period.
static MAX_FRESHNESS_SKEW_MS: u32 = 60 * 1000;

// --- Server types ---
pub type ServerSessionKey = SessionID;

#[derive(Debug)]
pub struct ServerSessionValue {
    pub sni: Option<webpki::DnsName>,
    pub version: ProtocolVersion,
    pub cipher_suite: CipherSuite,
    pub master_secret: PayloadU8,
    pub extended_ms: bool,
    pub client_cert_chain: Option<CertificatePayload>,
    pub alpn: Option<PayloadU8>,
    pub application_data: PayloadU16,
    pub creation_time_sec: u64,
    pub age_obfuscation_offset: u32,
    freshness: Option<bool>,
}

impl Codec for ServerSessionValue {
    fn encode(&self, bytes: &mut Vec<u8>) {
        if let Some(ref sni) = self.sni {
            1u8.encode(bytes);
            let sni_bytes: &str = sni.as_ref().into();
            PayloadU8::new(Vec::from(sni_bytes)).encode(bytes);
        } else {
            0u8.encode(bytes);
        }
        self.version.encode(bytes);
        self.cipher_suite.encode(bytes);
        self.master_secret.encode(bytes);
        (if self.extended_ms { 1u8 } else { 0u8 }).encode(bytes);
        if let Some(ref chain) = self.client_cert_chain {
            1u8.encode(bytes);
            chain.encode(bytes);
        } else {
            0u8.encode(bytes);
        }
        if let Some(ref alpn) = self.alpn {
            1u8.encode(bytes);
            alpn.encode(bytes);
        } else {
            0u8.encode(bytes);
        }
        self.application_data.encode(bytes);
        self.creation_time_sec.encode(bytes);
        self.age_obfuscation_offset
            .encode(bytes);
    }

    fn read(r: &mut Reader) -> Option<Self> {
        let has_sni = u8::read(r)?;
        let sni = if has_sni == 1 {
            let dns_name = PayloadU8::read(r)?;
            let dns_name = webpki::DnsNameRef::try_from_ascii(&dns_name.0).ok()?;
            Some(dns_name.into())
        } else {
            None
        };
        let v = ProtocolVersion::read(r)?;
        let cs = CipherSuite::read(r)?;
        let ms = PayloadU8::read(r)?;
        let ems = u8::read(r)?;
        let has_ccert = u8::read(r)? == 1;
        let ccert = if has_ccert {
            Some(CertificatePayload::read(r)?)
        } else {
            None
        };
        let has_alpn = u8::read(r)? == 1;
        let alpn = if has_alpn {
            Some(PayloadU8::read(r)?)
        } else {
            None
        };
        let application_data = PayloadU16::read(r)?;
        let creation_time_sec = u64::read(r)?;
        let age_obfuscation_offset = u32::read(r)?;

        Some(Self {
            sni,
            version: v,
            cipher_suite: cs,
            master_secret: ms,
            extended_ms: ems == 1u8,
            client_cert_chain: ccert,
            alpn,
            application_data,
            creation_time_sec,
            age_obfuscation_offset,
            freshness: None,
        })
    }
}

impl ServerSessionValue {
    pub fn new(
        sni: Option<&webpki::DnsName>,
        v: ProtocolVersion,
        cs: CipherSuite,
        ms: Vec<u8>,
        client_cert_chain: Option<CertificatePayload>,
        alpn: Option<Vec<u8>>,
        application_data: Vec<u8>,
        creation_time: TimeBase,
        age_obfuscation_offset: u32,
    ) -> Self {
        Self {
            sni: sni.cloned(),
            version: v,
            cipher_suite: cs,
            master_secret: PayloadU8::new(ms),
            extended_ms: false,
            client_cert_chain,
            alpn: alpn.map(PayloadU8::new),
            application_data: PayloadU16::new(application_data),
            creation_time_sec: creation_time.as_secs(),
            age_obfuscation_offset,
            freshness: None,
        }
    }

    pub fn set_extended_ms_used(&mut self) {
        self.extended_ms = true;
    }

    pub fn set_freshness(mut self, obfuscated_client_age_ms: u32, time_now: TimeBase) -> Self {
        let client_age_ms = obfuscated_client_age_ms.wrapping_sub(self.age_obfuscation_offset);
        let server_age_ms = (time_now
            .as_secs()
            .saturating_sub(self.creation_time_sec) as u32)
            .saturating_mul(1000);

        let age_difference = if client_age_ms < server_age_ms {
            server_age_ms - client_age_ms
        } else {
            client_age_ms - server_age_ms
        };

        self.freshness = Some(age_difference <= MAX_FRESHNESS_SKEW_MS);
        self
    }

    pub fn is_fresh(&self) -> bool {
        self.freshness.unwrap_or_default()
    }
}
