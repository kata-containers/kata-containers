//! OIDs commonly used with X.509 certificates

/// Unsafely marks a branch as unreachable.
/// This won't panic if reached, however children will be sacrificed and dark magic performed.
///
/// # Unsafety
///
/// This is incredibly unsafe.
/// You can already see Hades waving his hand at you from here.
/// You shall not pass this bridge leading to insanity. Never.
/// No one can tell you what would happen if you did.
/// Only one thing is for sure: it leads to a land of desolation called UB.
/// I mean, I'm literally creating infinity out of emptiness.
/// If you don't care about your mental sanity, you can read the
/// [nomicon on unchecked uninitialized memory](https://doc.rust-lang.org/nomicon/unchecked-uninit.html).
unsafe fn unreachable() -> ! {
    std::hint::unreachable_unchecked()
}

macro_rules! define_oid {
    ($uppercase:ident => $lowercase:ident => $str_value:literal) => {
        pub const $uppercase: &'static str = $str_value;

        pub fn $lowercase() -> ::oid::ObjectIdentifier {
            use ::std::sync::Once;

            static mut OID: Option<::oid::ObjectIdentifier> = None;
            static INIT: Once = Once::new();
            unsafe {
                INIT.call_once(|| {
                    OID = Some($uppercase.try_into().unwrap())
                });
                if let Some(oid) = &OID { oid.clone() } else { unreachable() }
            }
        }
    };
    ( $( $uppercase:ident => $lowercase:ident => $str_value:literal, )+ ) => {
        $( define_oid! { $uppercase => $lowercase => $str_value } )+
    };
}

define_oid! {
    // x9-57
    DSA_WITH_SHA1 => dsa_with_sha1 => "1.2.840.10040.4.3",
    // ANSI-X962
    EC_PUBLIC_KEY => ec_public_key => "1.2.840.10045.2.1",
    ECDSA_WITH_SHA256 => ecdsa_with_sha256 => "1.2.840.10045.4.3.2",
    ECDSA_WITH_SHA384 => ecdsa_with_sha384 => "1.2.840.10045.4.3.3",
    ECDSA_WITH_SHA512 => ecdsa_with_sha512 => "1.2.840.10045.4.3.4",
    SECP192R1 => secp192r1 => "1.2.840.10045.3.1.1",
    SECP256R1 => secp256r1 => "1.2.840.10045.3.1.7",

    // RSADSI
    RSA_ENCRYPTION => rsa_encryption => "1.2.840.113549.1.1.1",
    MD5_WITH_RSA_ENCRYPTHION => md5_with_rsa_encryption => "1.2.840.113549.1.1.4",
    SHA1_WITH_RSA_ENCRYPTION => sha1_with_rsa_encryption => "1.2.840.113549.1.1.5",
    SHA256_WITH_RSA_ENCRYPTION => sha256_with_rsa_encryption => "1.2.840.113549.1.1.11",
    SHA384_WITH_RSA_ENCRYPTION => sha384_with_rsa_encryption => "1.2.840.113549.1.1.12",
    SHA512_WITH_RSA_ENCRYPTION => sha512_with_rsa_encryption => "1.2.840.113549.1.1.13",
    SHA224_WITH_RSA_ENCRYPTION => sha224_with_rsa_encryption => "1.2.840.113549.1.1.14",
    EMAIL_ADDRESS => email_address => "1.2.840.113549.1.9.1", // deprecated
    EXTENSION_REQ => extension_request => "1.2.840.113549.1.9.14",

    // pkcs7
    PKCS7 => pkcs7 => "1.2.840.113549.1.7.1",
    SIGNED_DATA => signed_data => "1.2.840.113549.1.7.2",
    CONTENT_TYPE => content_type => "1.2.840.113549.1.9.3",
    MESSAGE_DIGEST => message_digest => "1.2.840.113549.1.9.4",

    // NIST
    DSA_WITH_SHA224 => dsa_with_sha224 => "2.16.840.1.101.3.4.3.1",
    DSA_WITH_SHA256 => dsa_with_sha256 => "2.16.840.1.101.3.4.3.2",
    DSA_WITH_SHA384 => dsa_with_sha384 => "2.16.840.1.101.3.4.3.3",
    DSA_WITH_SHA512 => dsa_with_sha512 => "2.16.840.1.101.3.4.3.4",
    ID_ECDSA_WITH_SHA3_256 => id_ecdsa_with_sha3_256 => "2.16.840.1.101.3.4.3.10",
    ID_RSASSA_PKCS1_V1_5_WITH_SHA3_224 => id_rsassa_pkcs1_v1_5_with_sha3_224 => "2.16.840.1.101.3.4.3.13",
    ID_RSASSA_PKCS1_V1_5_WITH_SHA3_256 => id_rsassa_pkcs1_v1_5_with_sha3_256 => "2.16.840.1.101.3.4.3.14",
    ID_RSASSA_PKCS1_V1_5_WITH_SHA3_384 => id_rsassa_pkcs1_v1_5_with_sha3_384 => "2.16.840.1.101.3.4.3.15",
    ID_RSASSA_PKCS1_V1_5_WITH_SHA3_512 => id_rsassa_pkcs1_v1_5_with_sha3_512 => "2.16.840.1.101.3.4.3.16",

    // Certicom Object Identifiers
    SECP384R1 => secp384r1 => "1.3.132.0.34",
    SECT163K1 => sect163k1 => "1.3.132.0.1",
    SECT163R2 => sect163r2 => "1.3.132.0.15",
    SECP224R1 => secp224r1 => "1.3.132.0.33",
    SECT233K1 => sect233k1 => "1.3.132.0.26",
    SECT233R1 => sect233r1 => "1.3.132.0.27",
    SECT283K1 => sect283k1 => "1.3.132.0.16",
    SECT283R1 => sect283r1 => "1.3.132.0.17",
    SECT409K1 => sect409k1 => "1.3.132.0.36",
    SECT409R1 => sect409r1 => "1.3.132.0.37",
    SECP521R1 => secp521r1 => "1.3.132.0.35",
    SECT571K1 => sect571k1 => "1.3.132.0.38",
    SECT571R1 => sect571r1 => "1.3.132.0.39",

    // RFC 8410
    X25519 => x25519 => "1.3.101.110",
    X448 => x448 => "1.3.101.111",
    ED25519 => ed25519 => "1.3.101.112",
    ED448 => ed448 => "1.3.101.113",

    // Extended key purpose OIDS
    KP_SERVER_AUTH => kp_server_auth => "1.3.6.1.5.5.7.3.1",
    KP_CLIENT_AUTH => kp_client_auth => "1.3.6.1.5.5.7.3.2",
    KP_CODE_SIGNING => kp_code_signing => "1.3.6.1.5.5.7.3.3",
    KP_EMAIL_PROTECTION => kp_email_protection => "1.3.6.1.5.5.7.3.4",
    KP_IPSEC_END_SYSTEM => kp_ipsec_end_system => "1.3.6.1.5.5.7.3.5",
    KP_IPSPEC_TUNNEL => kp_ipsec_tunnel => "1.3.6.1.5.5.7.3.6",
    KP_IPSEC_USER => kp_ipsec_user => "1.3.6.1.5.5.7.3.7",
    KP_TIME_STAMPING => kp_time_stamping => "1.3.6.1.5.5.7.3.8",
    KP_OCSP_SIGNING => kp_ocsp_signing => "1.3.6.1.5.5.7.3.9",
    KP_ANY_EXTENDED_KEY_USAGE => kp_any_extended_key_usage => "2.5.29.37.0",
    KP_LIFETIME_SIGNING  => kp_lifetime_signing => "1.3.6.1.4.1.311.10.3.13",

    // attribute types
    AT_COMMON_NAME => at_common_name => "2.5.4.3",
    AT_SURNAME => at_surname => "2.5.4.4",
    AT_SERIAL_NUMBER => at_serial_number => "2.5.4.5",
    AT_COUNTRY_NAME => at_country_name => "2.5.4.6",
    AT_LOCALITY_NAME => at_locality_name => "2.5.4.7",
    AT_STATE_OR_PROVINCE_NAME => at_state_or_province_name => "2.5.4.8",
    AT_STREET_NAME => at_street_name => "2.5.4.9",
    AT_ORGANIZATION_NAME => at_organization_name => "2.5.4.10",
    AT_ORGANIZATIONAL_UNIT_NAME => at_organizational_unit_name => "2.5.4.11",

    // certificate extensions
    SUBJECT_KEY_IDENTIFIER => subject_key_identifier => "2.5.29.14",
    KEY_USAGE => key_usage => "2.5.29.15",
    SUBJECT_ALTERNATIVE_NAME => subject_alternative_name => "2.5.29.17",
    ISSUER_ALTERNATIVE_NAME => issuer_alternative_name => "2.5.29.18",
    BASIC_CONSTRAINTS => basic_constraints => "2.5.29.19",
    CRL_NUMBER => crl_number => "2.5.29.20",
    AUTHORITY_KEY_IDENTIFIER => authority_key_identifier => "2.5.29.35",
    EXTENDED_KEY_USAGE => extended_key_usage => "2.5.29.37",

    // aes
    // aes-128
    AES128_ECB => aes128_ecb => "2.16.840.1.101.3.4.1.1",
    AES128_CBC => aes128_cbc => "2.16.840.1.101.3.4.1.2",
    AES128_OFB => aes128_ofb => "2.16.840.1.101.3.4.1.3",
    AES128_CFB => aes128_cfb => "2.16.840.1.101.3.4.1.4",
    AES128_WRAP => aes128_wrap => "2.16.840.1.101.3.4.1.5",
    AES128_GCM => aes128_gcm => "2.16.840.1.101.3.4.1.6",
    AES128_CCM => aes128_ccm => "2.16.840.1.101.3.4.1.7",
    AES128_WRAP_PAD => aes128_wrap_pad => "2.16.840.1.101.3.4.1.8",
    // aes-192
    AES192_ECB => aes192_ecb => "2.16.840.1.101.3.4.1.21",
    AES192_CBC => aes192_cbc => "2.16.840.1.101.3.4.1.22",
    AES192_OFB => aes192_ofb => "2.16.840.1.101.3.4.1.23",
    AES192_CFB => aes192_cfb => "2.16.840.1.101.3.4.1.24",
    AES192_WRAP => aes192_wrap => "2.16.840.1.101.3.4.1.25",
    AES192_GCM => aes192_gcm => "2.16.840.1.101.3.4.1.26",
    AES192_CCM => aes192_ccm => "2.16.840.1.101.3.4.1.27",
    AES192_WRAP_PAD => aes192_wrap_pad => "2.16.840.1.101.3.4.1.28",
    // aes-256
    AES256_ECB => aes256_ecb => "2.16.840.1.101.3.4.1.41",
    AES256_CBC => aes256_cbc => "2.16.840.1.101.3.4.1.42",
    AES256_OFB => aes256_ofb => "2.16.840.1.101.3.4.1.43",
    AES256_CFB => aes256_cfb => "2.16.840.1.101.3.4.1.44",
    AES256_WRAP => aes256_wrap => "2.16.840.1.101.3.4.1.45",
    AES256_GCM => aes256_gcm => "2.16.840.1.101.3.4.1.46",
    AES256_CCM => aes256_ccm => "2.16.840.1.101.3.4.1.47",
    AES256_WRAP_PAD => aes256_wrap_pad => "2.16.840.1.101.3.4.1.48",

    // hash algorithm
    MD5 => md5 => "1.2.840.113549.2.5",
    SHA1 => sha1 => "1.3.14.3.2.26",
    SHA256 => sha256 => "2.16.840.1.101.3.4.2.1",
    SHA384 => sha384 => "2.16.840.1.101.3.4.2.2",
    SHA512 => sha512 => "2.16.840.1.101.3.4.2.3",
    SHA224 => sha224 => "2.16.840.1.101.3.4.2.4",
    SHA512_224 => sha512_224 => "2.16.840.1.101.3.4.2.5",
    SHA512_256 => sha512_256 => "2.16.840.1.101.3.4.2.6",
    SHA3_224 => sha3_224 => "2.16.840.1.101.3.4.2.7",
    SHA3_256 => sha3_256 => "2.16.840.1.101.3.4.2.8",
    SHA3_384 => sha3_384 => "2.16.840.1.101.3.4.2.9",
    SHA3_512 => sha3_512 => "2.16.840.1.101.3.4.2.10",
    SHAKE128 => shake128 => "2.16.840.1.101.3.4.2.11",
    SHAKE256 => shake256 => "2.16.840.1.101.3.4.2.12",

    // authenticode
    SIGNING_TIME => signing_time => "1.2.840.113549.1.9.5",
    COUNTER_SIGN => counter_sign => "1.2.840.113549.1.9.6",
    SPC_INDIRECT_DATA_OBJID => spc_indirect_data_objid => "1.3.6.1.4.1.311.2.1.4",
    SPC_STATEMENT_TYPE => spc_statement_type => "1.3.6.1.4.1.311.2.1.11",
    SPC_SP_OPUS_INFO_OBJID => spc_sp_opus_info_objid => "1.3.6.1.4.1.311.2.1.12",
    SPC_PE_IMAGE_DATAOBJ => spc_pe_image_dataobj => "1.3.6.1.4.1.311.2.1.15",
    SPC_SIPINFO_OBJID => spc_sip_info_objid => "1.3.6.1.4.1.311.2.1.30",
    TIMESTAMP_REQUEST => timestamp_request => "1.3.6.1.4.1.311.3.2.1",
    MS_COUNTER_SIGN => ms_counter_signature => "1.3.6.1.4.1.311.3.3.1",

    // CTL
    CERT_TRUST_LIST => cert_trust_list => "1.3.6.1.4.1.311.10.1",
    ROOT_LIST_SIGNER => root_list_signer => "1.3.6.1.4.1.311.10.3.9",

    CERT_ENHKEY_USAGE_PROP_ID => cert_enhkey_usage_prop_id => "1.3.6.1.4.1.311.10.11.9",
    CERT_FRIENDLY_NAME_PROP_ID => cert_friendly_name_prop_id => "1.3.6.1.4.1.311.10.11.11",
    CERT_KEY_IDENTIFIER_PROP_ID => cert_key_identifier_prop_id => "1.3.6.1.4.1.311.10.11.20",
    CERT_SUBJECT_NAME_MD5_HASH_PROP_ID => cert_subject_name_md5_hash_prop_id => "1.3.6.1.4.1.311.10.11.29",
    CERT_ROOT_PROGRAM_CERT_POLICIES_PROP_ID => cert_root_program_cert_policies_prop_id => "1.3.6.1.4.1.311.10.11.83",
    CERT_AUTH_ROOT_SHA256_HASH_PROP_ID => cert_auto_root_sha256_hash_prop_id => "1.3.6.1.4.1.311.10.11.98",
    CERT_DISALLOWED_FILETIME_PROP_ID => cert_disallowed_filetime_prop_id => "1.3.6.1.4.1.311.10.11.104",
    CERT_ROOT_PROGRAM_CHAIN_POLICIES_PROP_ID => cert_root_program_chain_policies_prop_id => "1.3.6.1.4.1.311.10.11.105",
    DISALLOWED_ENHKEY_USAGE => disallowed_enhkey_usage => "1.3.6.1.4.1.311.10.11.122",
    UNKNOWN_RESERVED_PROP_ID_126 => unknown_reserved_prop_id_126 => "1.3.6.1.4.1.311.10.11.126",
    UNKNOWN_RESERVED_PROP_ID_127 => unknown_reserved_prop_id_127 => "1.3.6.1.4.1.311.10.11.127",

    AUTO_UPDATE_END_REVOCATION => auto_update_end_revocation => "1.3.6.1.4.1.311.60.3.2",

    // NLA protocols
    KRB5 => krb5 => "1.2.840.113554.1.2.2",
    MS_KRB5 => ms_krb5 => "1.2.840.48018.1.2.2",
    KRB5_USER_TO_USER => krb5_user_to_user => "1.2.840.113554.1.2.2.3",
    NTLM_SSP => ntlm_ssp => "1.3.6.1.4.1.311.2.2.10",
    SPNEGO => spnego => "1.3.6.1.5.5.2",
}
