use crate::check::inappropriate_handshake_message;
use crate::conn::{CommonState, ConnectionRandoms, State};
use crate::error::Error;
use crate::hash_hs::{HandshakeHash, HandshakeHashBuffer};
use crate::kx;
#[cfg(feature = "logging")]
use crate::log::{debug, trace, warn};
use crate::msgs::base::{Payload, PayloadU8};
use crate::msgs::ccs::ChangeCipherSpecPayload;
use crate::msgs::codec::Codec;
use crate::msgs::enums::KeyUpdateRequest;
use crate::msgs::enums::{AlertDescription, NamedGroup, ProtocolVersion};
use crate::msgs::enums::{ContentType, ExtensionType, HandshakeType, SignatureScheme};
use crate::msgs::handshake::ClientExtension;
use crate::msgs::handshake::DigitallySignedStruct;
use crate::msgs::handshake::EncryptedExtensions;
use crate::msgs::handshake::NewSessionTicketPayloadTLS13;
use crate::msgs::handshake::{CertificateEntry, CertificatePayloadTLS13};
use crate::msgs::handshake::{HandshakeMessagePayload, HandshakePayload};
use crate::msgs::handshake::{HasServerExtensions, ServerHelloPayload};
use crate::msgs::handshake::{PresharedKeyIdentity, PresharedKeyOffer};
use crate::msgs::message::{Message, MessagePayload};
use crate::msgs::persist;
use crate::tls13::key_schedule::{
    KeyScheduleEarly, KeyScheduleHandshake, KeySchedulePreHandshake, KeyScheduleTraffic,
};
use crate::tls13::Tls13CipherSuite;
use crate::verify;
#[cfg(feature = "quic")]
use crate::{conn::Protocol, msgs::base::PayloadU16, quic};
use crate::{sign, KeyLog};

use super::client_conn::ClientConnectionData;
use super::hs::ClientContext;
use crate::client::common::ServerCertDetails;
use crate::client::common::{ClientAuthDetails, ClientHelloDetails};
use crate::client::{hs, ClientConfig, ServerName, StoresClientSessions};

use crate::ticketer::TimeBase;
use ring::constant_time;

use crate::sign::{CertifiedKey, Signer};
use std::sync::Arc;

// Extensions we expect in plaintext in the ServerHello.
static ALLOWED_PLAINTEXT_EXTS: &[ExtensionType] = &[
    ExtensionType::KeyShare,
    ExtensionType::PreSharedKey,
    ExtensionType::SupportedVersions,
];

// Only the intersection of things we offer, and those disallowed
// in TLS1.3
static DISALLOWED_TLS13_EXTS: &[ExtensionType] = &[
    ExtensionType::ECPointFormats,
    ExtensionType::SessionTicket,
    ExtensionType::RenegotiationInfo,
    ExtensionType::ExtendedMasterSecret,
];

pub(super) fn handle_server_hello(
    config: Arc<ClientConfig>,
    cx: &mut ClientContext,
    server_hello: &ServerHelloPayload,
    mut resuming_session: Option<persist::Tls13ClientSessionValue>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    early_key_schedule: Option<KeyScheduleEarly>,
    hello: ClientHelloDetails,
    our_key_share: kx::KeyExchange,
    mut sent_tls13_fake_ccs: bool,
) -> hs::NextStateOrError {
    validate_server_hello(cx.common, server_hello)?;

    let their_key_share = server_hello
        .get_key_share()
        .ok_or_else(|| {
            cx.common
                .send_fatal_alert(AlertDescription::MissingExtension);
            Error::PeerMisbehavedError("missing key share".to_string())
        })?;

    if our_key_share.group() != their_key_share.group {
        return Err(cx
            .common
            .illegal_param("wrong group for key share"));
    }

    let key_schedule_pre_handshake = if let (Some(selected_psk), Some(early_key_schedule)) =
        (server_hello.get_psk_index(), early_key_schedule)
    {
        if let Some(ref resuming) = resuming_session {
            let resuming_suite = match suite.can_resume_from(resuming.suite()) {
                Some(resuming) => resuming,
                None => {
                    return Err(cx
                        .common
                        .illegal_param("server resuming incompatible suite"));
                }
            };

            // If the server varies the suite here, we will have encrypted early data with
            // the wrong suite.
            if cx.data.early_data.is_enabled() && resuming_suite != suite {
                return Err(cx
                    .common
                    .illegal_param("server varied suite with early data"));
            }

            if selected_psk != 0 {
                return Err(cx
                    .common
                    .illegal_param("server selected invalid psk"));
            }

            debug!("Resuming using PSK");
            // The key schedule has been initialized and set in fill_in_psk_binder()
        } else {
            return Err(Error::PeerMisbehavedError(
                "server selected unoffered psk".to_string(),
            ));
        }
        KeySchedulePreHandshake::from(early_key_schedule)
    } else {
        debug!("Not resuming");
        // Discard the early data key schedule.
        cx.data.early_data.rejected();
        cx.common.early_traffic = false;
        resuming_session.take();
        KeySchedulePreHandshake::new(suite.hkdf_algorithm)
    };

    let key_schedule = our_key_share.complete(&their_key_share.payload.0, |secret| {
        Ok(key_schedule_pre_handshake.into_handshake(secret))
    })?;

    // Remember what KX group the server liked for next time.
    save_kx_hint(&config, &server_name, their_key_share.group);

    // If we change keying when a subsequent handshake message is being joined,
    // the two halves will have different record layer protections.  Disallow this.
    cx.common.check_aligned_handshake()?;

    let hash_at_client_recvd_server_hello = transcript.get_current_hash();

    let (key_schedule, client_key, server_key) = key_schedule.derive_handshake_secrets(
        hash_at_client_recvd_server_hello,
        &*config.key_log,
        &randoms.client,
    );

    // Decrypt with the peer's key, encrypt with our own key
    cx.common
        .record_layer
        .set_message_decrypter(suite.derive_decrypter(&server_key));

    if !cx.data.early_data.is_enabled() {
        // Set the client encryption key for handshakes if early data is not used
        cx.common
            .record_layer
            .set_message_encrypter(suite.derive_encrypter(&client_key));
    }

    #[cfg(feature = "quic")]
    if cx.common.is_quic() {
        cx.common.quic.hs_secrets = Some(quic::Secrets::new(client_key, server_key, suite, true));
    }

    emit_fake_ccs(&mut sent_tls13_fake_ccs, cx.common);

    Ok(Box::new(ExpectEncryptedExtensions {
        config,
        resuming_session,
        server_name,
        randoms,
        suite,
        transcript,
        key_schedule,
        hello,
    }))
}

fn validate_server_hello(
    common: &mut CommonState,
    server_hello: &ServerHelloPayload,
) -> Result<(), Error> {
    for ext in &server_hello.extensions {
        if !ALLOWED_PLAINTEXT_EXTS.contains(&ext.get_type()) {
            common.send_fatal_alert(AlertDescription::UnsupportedExtension);
            return Err(Error::PeerMisbehavedError(
                "server sent unexpected cleartext ext".to_string(),
            ));
        }
    }

    Ok(())
}

pub(super) fn initial_key_share(
    config: &ClientConfig,
    server_name: &ServerName,
) -> Result<kx::KeyExchange, Error> {
    let key = persist::ClientSessionKey::hint_for_server_name(server_name);
    let key_buf = key.get_encoding();

    let maybe_value = config.session_storage.get(&key_buf);

    let group = maybe_value
        .and_then(|enc| NamedGroup::read_bytes(&enc))
        .and_then(|group| kx::KeyExchange::choose(group, &config.kx_groups))
        .unwrap_or_else(|| {
            config
                .kx_groups
                .first()
                .expect("No kx groups configured")
        });

    kx::KeyExchange::start(group).ok_or(Error::FailedToGetRandomBytes)
}

fn save_kx_hint(config: &ClientConfig, server_name: &ServerName, group: NamedGroup) {
    let key = persist::ClientSessionKey::hint_for_server_name(server_name);

    config
        .session_storage
        .put(key.get_encoding(), group.get_encoding());
}

/// This implements the horrifying TLS1.3 hack where PSK binders have a
/// data dependency on the message they are contained within.
pub(super) fn fill_in_psk_binder(
    resuming: &persist::Tls13ClientSessionValue,
    transcript: &HandshakeHashBuffer,
    hmp: &mut HandshakeMessagePayload,
) -> KeyScheduleEarly {
    // We need to know the hash function of the suite we're trying to resume into.
    let hkdf_alg = resuming.suite().hkdf_algorithm;
    let suite_hash = resuming.suite().hash_algorithm();

    // The binder is calculated over the clienthello, but doesn't include itself or its
    // length, or the length of its container.
    let binder_plaintext = hmp.get_encoding_for_binder_signing();
    let handshake_hash = transcript.get_hash_given(suite_hash, &binder_plaintext);

    // Run a fake key_schedule to simulate what the server will do if it chooses
    // to resume.
    let key_schedule = KeyScheduleEarly::new(hkdf_alg, resuming.secret());
    let real_binder = key_schedule.resumption_psk_binder_key_and_sign_verify_data(&handshake_hash);

    if let HandshakePayload::ClientHello(ref mut ch) = hmp.payload {
        ch.set_psk_binder(real_binder.as_ref());
    };

    key_schedule
}

pub(super) fn prepare_resumption(
    config: &ClientConfig,
    cx: &mut ClientContext<'_>,
    ticket: Vec<u8>,
    resuming_session: &persist::Retrieved<&persist::Tls13ClientSessionValue>,
    exts: &mut Vec<ClientExtension>,
    doing_retry: bool,
) {
    let resuming_suite = resuming_session.suite();
    cx.common.suite = Some(resuming_suite.into());
    cx.data.resumption_ciphersuite = Some(resuming_suite.into());
    // The EarlyData extension MUST be supplied together with the
    // PreSharedKey extension.
    let max_early_data_size = resuming_session.max_early_data_size();
    if config.enable_early_data && max_early_data_size > 0 && !doing_retry {
        cx.data
            .early_data
            .enable(max_early_data_size as usize);
        exts.push(ClientExtension::EarlyData);
    }

    // Finally, and only for TLS1.3 with a ticket resumption, include a binder
    // for our ticket.  This must go last.
    //
    // Include an empty binder. It gets filled in below because it depends on
    // the message it's contained in (!!!).
    let obfuscated_ticket_age = resuming_session.obfuscated_ticket_age();

    let binder_len = resuming_suite
        .hash_algorithm()
        .output_len;
    let binder = vec![0u8; binder_len];

    let psk_identity = PresharedKeyIdentity::new(ticket, obfuscated_ticket_age);
    let psk_ext = PresharedKeyOffer::new(psk_identity, binder);
    exts.push(ClientExtension::PresharedKey(psk_ext));
}

pub(super) fn derive_early_traffic_secret(
    key_log: &dyn KeyLog,
    cx: &mut ClientContext<'_>,
    resuming_suite: &'static Tls13CipherSuite,
    early_key_schedule: &KeyScheduleEarly,
    sent_tls13_fake_ccs: &mut bool,
    transcript_buffer: &HandshakeHashBuffer,
    client_random: &[u8; 32],
) {
    // For middlebox compatibility
    emit_fake_ccs(sent_tls13_fake_ccs, cx.common);

    let client_hello_hash = transcript_buffer.get_hash_given(resuming_suite.hash_algorithm(), &[]);
    let client_early_traffic_secret =
        early_key_schedule.client_early_traffic_secret(&client_hello_hash, key_log, client_random);
    // Set early data encryption key
    cx.common
        .record_layer
        .set_message_encrypter(resuming_suite.derive_encrypter(&client_early_traffic_secret));

    #[cfg(feature = "quic")]
    if cx.common.is_quic() {
        cx.common.quic.early_secret = Some(client_early_traffic_secret);
    }

    // Now the client can send encrypted early data
    cx.common.early_traffic = true;
    trace!("Starting early data traffic");
}

pub(super) fn emit_fake_ccs(sent_tls13_fake_ccs: &mut bool, common: &mut CommonState) {
    if common.is_quic() {
        return;
    }

    if std::mem::replace(sent_tls13_fake_ccs, true) {
        return;
    }

    let m = Message {
        version: ProtocolVersion::TLSv1_2,
        payload: MessagePayload::ChangeCipherSpec(ChangeCipherSpecPayload {}),
    };
    common.send_msg(m, false);
}

fn validate_encrypted_extensions(
    common: &mut CommonState,
    hello: &ClientHelloDetails,
    exts: &EncryptedExtensions,
) -> Result<(), Error> {
    if exts.has_duplicate_extension() {
        common.send_fatal_alert(AlertDescription::DecodeError);
        return Err(Error::PeerMisbehavedError(
            "server sent duplicate encrypted extensions".to_string(),
        ));
    }

    if hello.server_sent_unsolicited_extensions(exts, &[]) {
        common.send_fatal_alert(AlertDescription::UnsupportedExtension);
        let msg = "server sent unsolicited encrypted extension".to_string();
        return Err(Error::PeerMisbehavedError(msg));
    }

    for ext in exts {
        if ALLOWED_PLAINTEXT_EXTS.contains(&ext.get_type())
            || DISALLOWED_TLS13_EXTS.contains(&ext.get_type())
        {
            common.send_fatal_alert(AlertDescription::UnsupportedExtension);
            let msg = "server sent inappropriate encrypted extension".to_string();
            return Err(Error::PeerMisbehavedError(msg));
        }
    }

    Ok(())
}

struct ExpectEncryptedExtensions {
    config: Arc<ClientConfig>,
    resuming_session: Option<persist::Tls13ClientSessionValue>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    hello: ClientHelloDetails,
}

impl State<ClientConnectionData> for ExpectEncryptedExtensions {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let exts = require_handshake_msg!(
            m,
            HandshakeType::EncryptedExtensions,
            HandshakePayload::EncryptedExtensions
        )?;
        debug!("TLS1.3 encrypted extensions: {:?}", exts);
        self.transcript.add_message(&m);

        validate_encrypted_extensions(cx.common, &self.hello, exts)?;
        hs::process_alpn_protocol(cx.common, &self.config, exts.get_alpn_protocol())?;

        #[cfg(feature = "quic")]
        {
            // QUIC transport parameters
            if cx.common.is_quic() {
                match exts.get_quic_params_extension() {
                    Some(params) => cx.common.quic.params = Some(params),
                    None => {
                        return Err(cx
                            .common
                            .missing_extension("QUIC transport parameters not found"));
                    }
                }
            }
        }

        if let Some(resuming_session) = self.resuming_session {
            let was_early_traffic = cx.common.early_traffic;
            if was_early_traffic {
                if exts.early_data_extension_offered() {
                    cx.data.early_data.accepted();
                } else {
                    cx.data.early_data.rejected();
                    cx.common.early_traffic = false;
                }
            }

            if was_early_traffic && !cx.common.early_traffic {
                // If no early traffic, set the encryption key for handshakes
                cx.common
                    .record_layer
                    .set_message_encrypter(
                        self.suite
                            .derive_encrypter(self.key_schedule.client_key()),
                    );
            }

            cx.common.peer_certificates = Some(
                resuming_session
                    .server_cert_chain()
                    .to_vec(),
            );

            // We *don't* reverify the certificate chain here: resumption is a
            // continuation of the previous session in terms of security policy.
            let cert_verified = verify::ServerCertVerified::assertion();
            let sig_verified = verify::HandshakeSignatureValid::assertion();
            Ok(Box::new(ExpectFinished {
                config: self.config,
                server_name: self.server_name,
                randoms: self.randoms,
                suite: self.suite,
                transcript: self.transcript,
                key_schedule: self.key_schedule,
                client_auth: None,
                cert_verified,
                sig_verified,
            }))
        } else {
            if exts.early_data_extension_offered() {
                let msg = "server sent early data extension without resumption".to_string();
                return Err(Error::PeerMisbehavedError(msg));
            }
            Ok(Box::new(ExpectCertificateOrCertReq {
                config: self.config,
                server_name: self.server_name,
                randoms: self.randoms,
                suite: self.suite,
                transcript: self.transcript,
                key_schedule: self.key_schedule,
                may_send_sct_list: self.hello.server_may_send_sct_list(),
            }))
        }
    }
}

struct ExpectCertificateOrCertReq {
    config: Arc<ClientConfig>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    may_send_sct_list: bool,
}

impl State<ClientConnectionData> for ExpectCertificateOrCertReq {
    fn handle(self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        match m.payload {
            MessagePayload::Handshake {
                parsed:
                    HandshakeMessagePayload {
                        payload: HandshakePayload::CertificateTLS13(..),
                        ..
                    },
                ..
            } => Box::new(ExpectCertificate {
                config: self.config,
                server_name: self.server_name,
                randoms: self.randoms,
                suite: self.suite,
                transcript: self.transcript,
                key_schedule: self.key_schedule,
                may_send_sct_list: self.may_send_sct_list,
                client_auth: None,
            })
            .handle(cx, m),
            MessagePayload::Handshake {
                parsed:
                    HandshakeMessagePayload {
                        payload: HandshakePayload::CertificateRequestTLS13(..),
                        ..
                    },
                ..
            } => Box::new(ExpectCertificateRequest {
                config: self.config,
                server_name: self.server_name,
                randoms: self.randoms,
                suite: self.suite,
                transcript: self.transcript,
                key_schedule: self.key_schedule,
                may_send_sct_list: self.may_send_sct_list,
            })
            .handle(cx, m),
            payload => Err(inappropriate_handshake_message(
                &payload,
                &[ContentType::Handshake],
                &[
                    HandshakeType::Certificate,
                    HandshakeType::CertificateRequest,
                ],
            )),
        }
    }
}

// TLS1.3 version of CertificateRequest handling.  We then move to expecting the server
// Certificate. Unfortunately the CertificateRequest type changed in an annoying way
// in TLS1.3.
struct ExpectCertificateRequest {
    config: Arc<ClientConfig>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    may_send_sct_list: bool,
}

impl State<ClientConnectionData> for ExpectCertificateRequest {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let certreq = &require_handshake_msg!(
            m,
            HandshakeType::CertificateRequest,
            HandshakePayload::CertificateRequestTLS13
        )?;
        self.transcript.add_message(&m);
        debug!("Got CertificateRequest {:?}", certreq);

        // Fortunately the problems here in TLS1.2 and prior are corrected in
        // TLS1.3.

        // Must be empty during handshake.
        if !certreq.context.0.is_empty() {
            warn!("Server sent non-empty certreq context");
            cx.common
                .send_fatal_alert(AlertDescription::DecodeError);
            return Err(Error::CorruptMessagePayload(ContentType::Handshake));
        }

        let tls13_sign_schemes = sign::supported_sign_tls13();
        let no_sigschemes = Vec::new();
        let compat_sigschemes = certreq
            .get_sigalgs_extension()
            .unwrap_or(&no_sigschemes)
            .iter()
            .cloned()
            .filter(|scheme| tls13_sign_schemes.contains(scheme))
            .collect::<Vec<SignatureScheme>>();

        if compat_sigschemes.is_empty() {
            cx.common
                .send_fatal_alert(AlertDescription::HandshakeFailure);
            return Err(Error::PeerIncompatibleError(
                "server sent bad certreq schemes".to_string(),
            ));
        }

        let client_auth = ClientAuthDetails::resolve(
            self.config
                .client_auth_cert_resolver
                .as_ref(),
            certreq.get_authorities_extension(),
            &compat_sigschemes,
            Some(certreq.context.0.clone()),
        );

        Ok(Box::new(ExpectCertificate {
            config: self.config,
            server_name: self.server_name,
            randoms: self.randoms,
            suite: self.suite,
            transcript: self.transcript,
            key_schedule: self.key_schedule,
            may_send_sct_list: self.may_send_sct_list,
            client_auth: Some(client_auth),
        }))
    }
}

struct ExpectCertificate {
    config: Arc<ClientConfig>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    may_send_sct_list: bool,
    client_auth: Option<ClientAuthDetails>,
}

impl State<ClientConnectionData> for ExpectCertificate {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let cert_chain = require_handshake_msg!(
            m,
            HandshakeType::Certificate,
            HandshakePayload::CertificateTLS13
        )?;
        self.transcript.add_message(&m);

        // This is only non-empty for client auth.
        if !cert_chain.context.0.is_empty() {
            warn!("certificate with non-empty context during handshake");
            cx.common
                .send_fatal_alert(AlertDescription::DecodeError);
            return Err(Error::CorruptMessagePayload(ContentType::Handshake));
        }

        if cert_chain.any_entry_has_duplicate_extension()
            || cert_chain.any_entry_has_unknown_extension()
        {
            warn!("certificate chain contains unsolicited/unknown extension");
            cx.common
                .send_fatal_alert(AlertDescription::UnsupportedExtension);
            return Err(Error::PeerMisbehavedError(
                "bad cert chain extensions".to_string(),
            ));
        }

        let server_cert = ServerCertDetails::new(
            cert_chain.convert(),
            cert_chain.get_end_entity_ocsp(),
            cert_chain.get_end_entity_scts(),
        );

        if let Some(sct_list) = server_cert.scts.as_ref() {
            if hs::sct_list_is_invalid(sct_list) {
                let error_msg = "server sent invalid SCT list".to_string();
                return Err(Error::PeerMisbehavedError(error_msg));
            }

            if !self.may_send_sct_list {
                let error_msg = "server sent unsolicited SCT list".to_string();
                return Err(Error::PeerMisbehavedError(error_msg));
            }
        }

        Ok(Box::new(ExpectCertificateVerify {
            config: self.config,
            server_name: self.server_name,
            randoms: self.randoms,
            suite: self.suite,
            transcript: self.transcript,
            key_schedule: self.key_schedule,
            server_cert,
            client_auth: self.client_auth,
        }))
    }
}

// --- TLS1.3 CertificateVerify ---
struct ExpectCertificateVerify {
    config: Arc<ClientConfig>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    server_cert: ServerCertDetails,
    client_auth: Option<ClientAuthDetails>,
}

impl State<ClientConnectionData> for ExpectCertificateVerify {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let cert_verify = require_handshake_msg!(
            m,
            HandshakeType::CertificateVerify,
            HandshakePayload::CertificateVerify
        )?;

        trace!("Server cert is {:?}", self.server_cert.cert_chain);

        // 1. Verify the certificate chain.
        let (end_entity, intermediates) = self
            .server_cert
            .cert_chain
            .split_first()
            .ok_or(Error::NoCertificatesPresented)?;
        let now = std::time::SystemTime::now();
        let cert_verified = self
            .config
            .verifier
            .verify_server_cert(
                end_entity,
                intermediates,
                &self.server_name,
                &mut self.server_cert.scts(),
                &self.server_cert.ocsp_response,
                now,
            )
            .map_err(|err| hs::send_cert_error_alert(cx.common, err))?;

        // 2. Verify their signature on the handshake.
        let handshake_hash = self.transcript.get_current_hash();
        let sig_verified = self
            .config
            .verifier
            .verify_tls13_signature(
                &verify::construct_tls13_server_verify_message(&handshake_hash),
                &self.server_cert.cert_chain[0],
                cert_verify,
            )
            .map_err(|err| hs::send_cert_error_alert(cx.common, err))?;

        cx.common.peer_certificates = Some(self.server_cert.cert_chain);
        self.transcript.add_message(&m);

        Ok(Box::new(ExpectFinished {
            config: self.config,
            server_name: self.server_name,
            randoms: self.randoms,
            suite: self.suite,
            transcript: self.transcript,
            key_schedule: self.key_schedule,
            client_auth: self.client_auth,
            cert_verified,
            sig_verified,
        }))
    }
}

fn emit_certificate_tls13(
    transcript: &mut HandshakeHash,
    certkey: Option<&CertifiedKey>,
    auth_context: Option<Vec<u8>>,
    common: &mut CommonState,
) {
    let context = auth_context.unwrap_or_default();

    let mut cert_payload = CertificatePayloadTLS13 {
        context: PayloadU8::new(context),
        entries: Vec::new(),
    };

    if let Some(certkey) = certkey {
        for cert in &certkey.cert {
            cert_payload
                .entries
                .push(CertificateEntry::new(cert.clone()));
        }
    }

    let m = Message {
        version: ProtocolVersion::TLSv1_3,
        payload: MessagePayload::handshake(HandshakeMessagePayload {
            typ: HandshakeType::Certificate,
            payload: HandshakePayload::CertificateTLS13(cert_payload),
        }),
    };
    transcript.add_message(&m);
    common.send_msg(m, true);
}

fn emit_certverify_tls13(
    transcript: &mut HandshakeHash,
    signer: &dyn Signer,
    common: &mut CommonState,
) -> Result<(), Error> {
    let message = verify::construct_tls13_client_verify_message(&transcript.get_current_hash());

    let scheme = signer.scheme();
    let sig = signer.sign(&message)?;
    let dss = DigitallySignedStruct::new(scheme, sig);

    let m = Message {
        version: ProtocolVersion::TLSv1_3,
        payload: MessagePayload::handshake(HandshakeMessagePayload {
            typ: HandshakeType::CertificateVerify,
            payload: HandshakePayload::CertificateVerify(dss),
        }),
    };

    transcript.add_message(&m);
    common.send_msg(m, true);
    Ok(())
}

fn emit_finished_tls13(
    transcript: &mut HandshakeHash,
    verify_data: ring::hmac::Tag,
    common: &mut CommonState,
) {
    let verify_data_payload = Payload::new(verify_data.as_ref());

    let m = Message {
        version: ProtocolVersion::TLSv1_3,
        payload: MessagePayload::handshake(HandshakeMessagePayload {
            typ: HandshakeType::Finished,
            payload: HandshakePayload::Finished(verify_data_payload),
        }),
    };

    transcript.add_message(&m);
    common.send_msg(m, true);
}

fn emit_end_of_early_data_tls13(transcript: &mut HandshakeHash, common: &mut CommonState) {
    if common.is_quic() {
        return;
    }

    let m = Message {
        version: ProtocolVersion::TLSv1_3,
        payload: MessagePayload::handshake(HandshakeMessagePayload {
            typ: HandshakeType::EndOfEarlyData,
            payload: HandshakePayload::EndOfEarlyData,
        }),
    };

    transcript.add_message(&m);
    common.send_msg(m, true);
}

struct ExpectFinished {
    config: Arc<ClientConfig>,
    server_name: ServerName,
    randoms: ConnectionRandoms,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleHandshake,
    client_auth: Option<ClientAuthDetails>,
    cert_verified: verify::ServerCertVerified,
    sig_verified: verify::HandshakeSignatureValid,
}

impl State<ClientConnectionData> for ExpectFinished {
    fn handle(self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let mut st = *self;
        let finished =
            require_handshake_msg!(m, HandshakeType::Finished, HandshakePayload::Finished)?;

        let handshake_hash = st.transcript.get_current_hash();
        let expect_verify_data = st
            .key_schedule
            .sign_server_finish(&handshake_hash);

        let fin = constant_time::verify_slices_are_equal(expect_verify_data.as_ref(), &finished.0)
            .map_err(|_| {
                cx.common
                    .send_fatal_alert(AlertDescription::DecryptError);
                Error::DecryptError
            })
            .map(|_| verify::FinishedMessageVerified::assertion())?;

        st.transcript.add_message(&m);

        let hash_after_handshake = st.transcript.get_current_hash();
        /* The EndOfEarlyData message to server is still encrypted with early data keys,
         * but appears in the transcript after the server Finished. */
        if cx.common.early_traffic {
            emit_end_of_early_data_tls13(&mut st.transcript, cx.common);
            cx.common.early_traffic = false;
            cx.data.early_data.finished();
            cx.common
                .record_layer
                .set_message_encrypter(
                    st.suite
                        .derive_encrypter(st.key_schedule.client_key()),
                );
        }

        /* Send our authentication/finished messages.  These are still encrypted
         * with our handshake keys. */
        if let Some(client_auth) = st.client_auth {
            match client_auth {
                ClientAuthDetails::Empty {
                    auth_context_tls13: auth_context,
                } => {
                    emit_certificate_tls13(&mut st.transcript, None, auth_context, cx.common);
                }
                ClientAuthDetails::Verify {
                    certkey,
                    signer,
                    auth_context_tls13: auth_context,
                } => {
                    emit_certificate_tls13(
                        &mut st.transcript,
                        Some(&certkey),
                        auth_context,
                        cx.common,
                    );
                    emit_certverify_tls13(&mut st.transcript, signer.as_ref(), cx.common)?;
                }
            }
        }

        let (key_schedule_finished, client_key, server_key) = st
            .key_schedule
            .into_traffic_with_client_finished_pending(
                hash_after_handshake,
                &*st.config.key_log,
                &st.randoms.client,
            );
        let handshake_hash = st.transcript.get_current_hash();
        let (key_schedule_traffic, verify_data, _) =
            key_schedule_finished.sign_client_finish(&handshake_hash);
        emit_finished_tls13(&mut st.transcript, verify_data, cx.common);

        /* Now move to our application traffic keys. */
        cx.common.check_aligned_handshake()?;

        cx.common
            .record_layer
            .set_message_decrypter(st.suite.derive_decrypter(&server_key));

        cx.common
            .record_layer
            .set_message_encrypter(st.suite.derive_encrypter(&client_key));

        cx.common.start_traffic();

        let st = ExpectTraffic {
            session_storage: Arc::clone(&st.config.session_storage),
            server_name: st.server_name,
            suite: st.suite,
            transcript: st.transcript,
            key_schedule: key_schedule_traffic,
            want_write_key_update: false,
            _cert_verified: st.cert_verified,
            _sig_verified: st.sig_verified,
            _fin_verified: fin,
        };

        #[cfg(feature = "quic")]
        {
            if cx.common.protocol == Protocol::Quic {
                cx.common.quic.traffic_secrets =
                    Some(quic::Secrets::new(client_key, server_key, st.suite, true));
                return Ok(Box::new(ExpectQuicTraffic(st)));
            }
        }

        Ok(Box::new(st))
    }
}

// -- Traffic transit state (TLS1.3) --
// In this state we can be sent tickets, key updates,
// and application data.
struct ExpectTraffic {
    session_storage: Arc<dyn StoresClientSessions>,
    server_name: ServerName,
    suite: &'static Tls13CipherSuite,
    transcript: HandshakeHash,
    key_schedule: KeyScheduleTraffic,
    want_write_key_update: bool,
    _cert_verified: verify::ServerCertVerified,
    _sig_verified: verify::HandshakeSignatureValid,
    _fin_verified: verify::FinishedMessageVerified,
}

impl ExpectTraffic {
    #[allow(clippy::unnecessary_wraps)] // returns Err for #[cfg(feature = "quic")]
    fn handle_new_ticket_tls13(
        &mut self,
        cx: &mut ClientContext<'_>,
        nst: &NewSessionTicketPayloadTLS13,
    ) -> Result<(), Error> {
        if nst.has_duplicate_extension() {
            cx.common
                .send_fatal_alert(AlertDescription::IllegalParameter);
            return Err(Error::PeerMisbehavedError(
                "peer sent duplicate NewSessionTicket extensions".into(),
            ));
        }

        let handshake_hash = self.transcript.get_current_hash();
        let secret = self
            .key_schedule
            .resumption_master_secret_and_derive_ticket_psk(&handshake_hash, &nst.nonce.0);

        let time_now = match TimeBase::now() {
            Ok(t) => t,
            #[allow(unused_variables)]
            Err(e) => {
                debug!("Session not saved: {}", e);
                return Ok(());
            }
        };

        let value = persist::Tls13ClientSessionValue::new(
            self.suite,
            nst.ticket.0.clone(),
            secret,
            cx.common
                .peer_certificates
                .clone()
                .unwrap_or_default(),
            time_now,
            nst.lifetime,
            nst.age_add,
            nst.get_max_early_data_size()
                .unwrap_or_default(),
        );

        #[cfg(feature = "quic")]
        if let Some(sz) = nst.get_max_early_data_size() {
            if cx.common.protocol == Protocol::Quic && sz != 0 && sz != 0xffff_ffff {
                return Err(Error::PeerMisbehavedError(
                    "invalid max_early_data_size".into(),
                ));
            }
        }

        let key = persist::ClientSessionKey::session_for_server_name(&self.server_name);
        #[allow(unused_mut)]
        let mut ticket = value.get_encoding();

        #[cfg(feature = "quic")]
        if let (Protocol::Quic, Some(ref quic_params)) =
            (cx.common.protocol, &cx.common.quic.params)
        {
            PayloadU16::encode_slice(quic_params, &mut ticket);
        }

        let worked = self
            .session_storage
            .put(key.get_encoding(), ticket);

        if worked {
            debug!("Ticket saved");
        } else {
            debug!("Ticket not saved");
        }
        Ok(())
    }

    fn handle_key_update(
        &mut self,
        common: &mut CommonState,
        kur: &KeyUpdateRequest,
    ) -> Result<(), Error> {
        #[cfg(feature = "quic")]
        {
            if let Protocol::Quic = common.protocol {
                common.send_fatal_alert(AlertDescription::UnexpectedMessage);
                let msg = "KeyUpdate received in QUIC connection".to_string();
                warn!("{}", msg);
                return Err(Error::PeerMisbehavedError(msg));
            }
        }

        // Mustn't be interleaved with other handshake messages.
        common.check_aligned_handshake()?;

        match kur {
            KeyUpdateRequest::UpdateNotRequested => {}
            KeyUpdateRequest::UpdateRequested => {
                self.want_write_key_update = true;
            }
            _ => {
                common.send_fatal_alert(AlertDescription::IllegalParameter);
                return Err(Error::CorruptMessagePayload(ContentType::Handshake));
            }
        }

        // Update our read-side keys.
        let new_read_key = self
            .key_schedule
            .next_server_application_traffic_secret();
        common
            .record_layer
            .set_message_decrypter(
                self.suite
                    .derive_decrypter(&new_read_key),
            );

        Ok(())
    }
}

impl State<ClientConnectionData> for ExpectTraffic {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        match m.payload {
            MessagePayload::ApplicationData(payload) => cx
                .common
                .take_received_plaintext(payload),
            MessagePayload::Handshake {
                parsed:
                    HandshakeMessagePayload {
                        payload: HandshakePayload::NewSessionTicketTLS13(ref new_ticket),
                        ..
                    },
                ..
            } => self.handle_new_ticket_tls13(cx, new_ticket)?,
            MessagePayload::Handshake {
                parsed:
                    HandshakeMessagePayload {
                        payload: HandshakePayload::KeyUpdate(ref key_update),
                        ..
                    },
                ..
            } => self.handle_key_update(cx.common, key_update)?,
            payload => {
                return Err(inappropriate_handshake_message(
                    &payload,
                    &[ContentType::ApplicationData, ContentType::Handshake],
                    &[HandshakeType::NewSessionTicket, HandshakeType::KeyUpdate],
                ));
            }
        }

        Ok(self)
    }

    fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        self.key_schedule
            .export_keying_material(output, label, context)
    }

    fn perhaps_write_key_update(&mut self, common: &mut CommonState) {
        if self.want_write_key_update {
            self.want_write_key_update = false;
            common.send_msg_encrypt(Message::build_key_update_notify().into());

            let write_key = self
                .key_schedule
                .next_client_application_traffic_secret();
            common
                .record_layer
                .set_message_encrypter(self.suite.derive_encrypter(&write_key));
        }
    }
}

#[cfg(feature = "quic")]
struct ExpectQuicTraffic(ExpectTraffic);

#[cfg(feature = "quic")]
impl State<ClientConnectionData> for ExpectQuicTraffic {
    fn handle(mut self: Box<Self>, cx: &mut ClientContext<'_>, m: Message) -> hs::NextStateOrError {
        let nst = require_handshake_msg!(
            m,
            HandshakeType::NewSessionTicket,
            HandshakePayload::NewSessionTicketTLS13
        )?;
        self.0
            .handle_new_ticket_tls13(cx, nst)?;
        Ok(self)
    }

    fn export_keying_material(
        &self,
        output: &mut [u8],
        label: &[u8],
        context: Option<&[u8]>,
    ) -> Result<(), Error> {
        self.0
            .export_keying_material(output, label, context)
    }
}
