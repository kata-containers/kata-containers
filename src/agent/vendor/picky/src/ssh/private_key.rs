use crate::key::{KeyError, PrivateKey};
use crate::pem::{parse_pem, Pem, PemError};
use crate::ssh::decode::{SshComplexTypeDecode, SshReadExt};
use crate::ssh::encode::SshComplexTypeEncode;
use crate::ssh::public_key::{SshBasePublicKey, SshPublicKey, SshPublicKeyError};
use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockDecryptMut, KeyIvInit, StreamCipher};
use byteorder::{BigEndian, ReadBytesExt};
use rand::Rng;
use std::io::{Cursor, Read};
use std::string;
use thiserror::Error;

pub type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
pub type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
pub type Aes128Ctr = ctr::Ctr32BE<aes::Aes128>;
pub type Aes256Ctr = ctr::Ctr32BE<aes::Aes256>;

const SSH_PRIVATE_KEY_LABEL: &str = "OPENSSH PRIVATE KEY";
pub(crate) const AUTH_MAGIC: &str = "openssh-key-v1";

const AES128_CTR: &str = "aes128-ctr";
pub(crate) const AES256_CTR: &str = "aes256-ctr";

const AES128_CBC: &str = "aes128-cbc";
const AES256_CBC: &str = "aes256-cbc";

pub(crate) const BCRYPT: &str = "bcrypt";
pub(crate) const NONE: &str = "none";

#[derive(Debug, Error)]
pub enum SshPrivateKeyError {
    #[error(transparent)]
    FromUtf8Error(#[from] string::FromUtf8Error),
    #[error(transparent)]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Unsupported key type: {0}")]
    UnsupportedKeyType(String),
    #[error("Unsupported cipher: {0}")]
    UnsupportedCipher(String),
    #[error("Unsupported kdf: {0}")]
    UnsupportedKdf(String),
    #[error("Invalid auth magic header")]
    InvalidAuthMagicHeader,
    #[error("Invalid keys amount. Expected 1 but got {0}")]
    InvalidKeysAmount(u32),
    #[error("Check numbers are not equal: {0} {1}. Wrong passphrase or key is corrupted")]
    InvalidCheckNumbers(u32, u32),
    #[error("Invalid public key: {0:?}")]
    InvalidPublicKey(#[from] SshPublicKeyError),
    #[error("Invalid key format")]
    InvalidKeyFormat,
    #[error("Can not decrypt private key: {0}")]
    DecryptionError(String),
    #[error("Can not hash the passphrase: {0:?}")]
    HashingError(#[from] bcrypt_pbkdf::Error),
    #[error("Passphrase required for encrypted private key")]
    MissingPassphrase,
    #[error(transparent)]
    KeyError(#[from] KeyError),
    #[error(transparent)]
    PemError(#[from] PemError),
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct KdfOption {
    pub salt: Vec<u8>,
    pub rounds: u32,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Kdf {
    pub name: String,
    pub option: KdfOption,
}

impl Default for Kdf {
    fn default() -> Self {
        Self {
            name: NONE.to_owned(),
            option: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SshBasePrivateKey {
    Rsa(PrivateKey),
}

impl SshBasePrivateKey {
    pub fn base_public_key(&self) -> SshBasePublicKey {
        match self {
            SshBasePrivateKey::Rsa(rsa) => SshBasePublicKey::Rsa(rsa.to_public_key()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SshPrivateKey {
    pub cipher_name: String,
    pub kdf: Kdf,
    pub base_key: SshBasePrivateKey,
    pub public_key: SshPublicKey,
    pub check: u32,
    pub comment: String,
    pub passphrase: Option<String>,
}

impl SshPrivateKey {
    pub fn generate_rsa(
        bits: usize,
        passphrase: Option<String>,
        comment: Option<String>,
    ) -> Result<Self, SshPrivateKeyError> {
        Ok(SshPrivateKey::h_picky_private_key_to_ssh_private_key(
            PrivateKey::generate_rsa(bits)?,
            passphrase,
            comment,
        ))
    }

    pub fn from_pem(pem: &Pem, passphrase: Option<String>) -> Result<Self, SshPrivateKeyError> {
        SshPrivateKey::decode(&mut pem.data(), passphrase)
    }

    pub fn from_pem_str(pem: &str, passphrase: Option<String>) -> Result<Self, SshPrivateKeyError> {
        let pem = parse_pem(pem)?;
        SshPrivateKey::decode(&mut pem.data(), passphrase)
    }

    pub fn to_pem(&self) -> Result<Pem<'static>, SshPrivateKeyError> {
        let mut buffer = Vec::with_capacity(2048);
        self.encode(&mut buffer)?;
        Ok(Pem::new(SSH_PRIVATE_KEY_LABEL, buffer))
    }

    pub fn to_string(&self) -> Result<String, SshPrivateKeyError> {
        let mut buffer = Vec::with_capacity(2048);
        self.encode(&mut buffer)?;
        let mut result = Pem::new(SSH_PRIVATE_KEY_LABEL, buffer).to_string();
        // ssh private key must contain \x0A (\n) character at the end
        result.push('\x0A');
        Ok(result)
    }

    pub fn public_key(&self) -> &SshPublicKey {
        &self.public_key
    }

    pub fn base_key(&self) -> &SshBasePrivateKey {
        &self.base_key
    }

    fn h_picky_private_key_to_ssh_private_key(
        private_key: PrivateKey,
        passphrase: Option<String>,
        comment: Option<String>,
    ) -> SshPrivateKey {
        let (kdf, cipher_name) = match &passphrase {
            Some(_) => {
                let mut salt = Vec::new();
                let rounds = 16;
                let mut rnd = rand::thread_rng();
                for _ in 0..rounds {
                    salt.push(rnd.gen::<u8>());
                }

                let kdf = Kdf {
                    name: BCRYPT.to_owned(),
                    option: KdfOption { salt, rounds },
                };

                (kdf, String::new())
            }
            None => (Kdf::default(), NONE.to_owned()),
        };

        let public_key = SshPublicKey {
            inner_key: SshBasePublicKey::Rsa(private_key.to_public_key()),
            comment: String::new(),
        };

        let base_key = SshBasePrivateKey::Rsa(private_key);

        SshPrivateKey {
            cipher_name,
            kdf,
            base_key,
            public_key,
            check: 0,
            comment: comment.unwrap_or_default(),
            passphrase,
        }
    }

    fn decode(mut stream: impl Read, passphrase: Option<String>) -> Result<Self, SshPrivateKeyError>
    where
        Self: Sized,
    {
        let mut auth_magic = [0; AUTH_MAGIC.as_bytes().len()];
        stream.read_exact(&mut auth_magic)?;
        if auth_magic != AUTH_MAGIC.as_bytes() {
            return Err(SshPrivateKeyError::InvalidAuthMagicHeader);
        }
        stream.read_u8()?; // skip 1 byte (null-byte)

        let cipher_name = stream.read_ssh_string()?;
        let kdf_name = stream.read_ssh_string()?;
        let kdf_option: KdfOption = SshComplexTypeDecode::decode(&mut stream)?;
        let keys_amount = stream.read_u32::<BigEndian>()?;

        if keys_amount != 1 {
            return Err(SshPrivateKeyError::InvalidKeysAmount(keys_amount));
        }

        // read public key
        let _ = stream.read_ssh_bytes()?;

        // read private key
        let private_key = stream.read_ssh_bytes()?;

        let data = decrypt(&cipher_name, &kdf_name, &kdf_option, passphrase.as_deref(), private_key)?;

        let mut cursor = Cursor::new(data);

        let check0 = cursor.read_u32::<BigEndian>()?;
        let check1 = cursor.read_u32::<BigEndian>()?;
        if check0 != check1 {
            return Err(SshPrivateKeyError::InvalidCheckNumbers(check0, check1));
        }

        let base_key: SshBasePrivateKey = SshComplexTypeDecode::decode(&mut cursor)?;
        let base_public_key = base_key.base_public_key();

        let comment = cursor.read_ssh_string()?.trim_end().to_owned();
        Ok(SshPrivateKey {
            base_key,
            public_key: SshPublicKey {
                inner_key: base_public_key,
                comment: String::new(),
            },
            passphrase,
            kdf: Kdf {
                name: kdf_name,
                option: kdf_option,
            },
            cipher_name,
            check: check0,
            comment,
        })
    }
}

impl From<PrivateKey> for SshPrivateKey {
    fn from(private_key: PrivateKey) -> Self {
        SshPrivateKey::h_picky_private_key_to_ssh_private_key(private_key, None, None)
    }
}

pub(crate) fn decrypt(
    cipher_name: &str,
    kdf_name: &str,
    kdf_options: &KdfOption,
    passphrase: Option<&str>,
    mut data: Vec<u8>,
) -> Result<Vec<u8>, SshPrivateKeyError> {
    if kdf_name == NONE {
        Ok(data)
    } else {
        let n = match cipher_name {
            AES128_CBC | AES128_CTR => 32,
            AES256_CBC | AES256_CTR => 48,
            name => return Err(SshPrivateKeyError::UnsupportedCipher(name.to_owned())),
        };

        let mut key = [0; 48];
        match kdf_name {
            BCRYPT => {
                let salt = &kdf_options.salt;
                let rounds = kdf_options.rounds;
                let passphrase = passphrase.ok_or(SshPrivateKeyError::MissingPassphrase)?;

                bcrypt_pbkdf::bcrypt_pbkdf(passphrase, salt, rounds, &mut key[..n])?;
            }
            name => return Err(SshPrivateKeyError::UnsupportedKdf(name.to_owned())),
        };

        let (key, iv) = key.split_at(n - 16);

        let start_len = data.len();
        data.resize(data.len() + 32, 0u8);
        match cipher_name {
            AES128_CBC => {
                let cipher = Aes128CbcDec::new_from_slices(key, iv).unwrap();
                let n = cipher
                    .decrypt_padded_mut::<NoPadding>(&mut data)
                    .map_err(|e| SshPrivateKeyError::DecryptionError(e.to_string()))?
                    .len();
                data.truncate(n);
                Ok(data)
            }
            AES256_CBC => {
                let cipher = Aes256CbcDec::new_from_slices(key, iv).unwrap();
                let n = cipher
                    .decrypt_padded_mut::<NoPadding>(&mut data)
                    .map_err(|e| SshPrivateKeyError::DecryptionError(e.to_string()))?
                    .len();
                data.truncate(n);
                Ok(data)
            }
            AES128_CTR => {
                let mut cipher = Aes128Ctr::new_from_slices(key, iv).unwrap();
                cipher.apply_keystream(&mut data);
                data.truncate(start_len);
                Ok(data)
            }
            AES256_CTR => {
                let mut cipher = Aes256Ctr::new_from_slices(key, iv).unwrap();
                cipher.apply_keystream(&mut data);
                data.truncate(start_len);
                Ok(data)
            }
            name => Err(SshPrivateKeyError::UnsupportedCipher(name.to_owned())),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::ssh::private_key::SshPrivateKey;

    #[test]
    fn decode_without_passphrase_2048() {
        // ssh-keygen -t rsa -b 2048 -C "test2@picky.com" (without the passphrase)
        let ssh_private_key_pem = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                                        b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdz\n\
                                        c2gtcnNhAAAAAwEAAQAAAQEAyPYbdoNqjj4EhuYblWIxVKLsmsOff+kLkKlFRsIJ\n\
                                        yE5YUWzPm5LyUH3LoqnL/rw/f/Og37oJO/bEn4P2lSvlf6ZagAGaLo8/8ACw4xKY\n\
                                        UsQFHAEfreIthd/T2u9TEnN+yPS99M99bXG2tV+6He4c61TJfYrq5DsgQuMXCFmt\n\
                                        R/IdJg8qF8lj06qEzjQ1HvXQdXruhm4sQn1HMb3VbdKQFSU3TpmzVysEaOVl3zK7\n\
                                        KirBU9gHIOFZuE3y0oUklFuK6jOhjgQnxeo58Rb00g3p7R+YcpI1i95TAoIQ/tYS\n\
                                        cjnZzByQv+ak1BjgfOjMbEeEQl6kvi2axqTEnFcg0IHu6wAAA8iqDGUDqgxlAwAA\n\
                                        AAdzc2gtcnNhAAABAQDI9ht2g2qOPgSG5huVYjFUouyaw59/6QuQqUVGwgnITlhR\n\
                                        bM+bkvJQfcuiqcv+vD9/86Dfugk79sSfg/aVK+V/plqAAZoujz/wALDjEphSxAUc\n\
                                        AR+t4i2F39Pa71MSc37I9L30z31tcba1X7od7hzrVMl9iurkOyBC4xcIWa1H8h0m\n\
                                        DyoXyWPTqoTONDUe9dB1eu6GbixCfUcxvdVt0pAVJTdOmbNXKwRo5WXfMrsqKsFT\n\
                                        2Acg4Vm4TfLShSSUW4rqM6GOBCfF6jnxFvTSDentH5hykjWL3lMCghD+1hJyOdnM\n\
                                        HJC/5qTUGOB86MxsR4RCXqS+LZrGpMScVyDQge7rAAAAAwEAAQAAAQATZEw6H2xE\n\
                                        1Y8yRTocLCF+fUo/lOjrOt22096veUHgZk73bHyMEp33Tmw8Ag6BQkEOY7/+VsFV\n\
                                        W/aVPfKpalb2/mJ1P7JVE9Wjny1ye/Te57NmhGU+LjkeVf7nfXiSqzpswdEisnL0\n\
                                        AKkUz2vyP2vi+YeH6cPIyjvOuIMcdyrVakejnGbss19ZoXw660X/7TRqG/41KhTm\n\
                                        lkN610JBKI2Rozecx9l3LZ3CTRpOOJ2sfssegvL+qxvvH1YVkRat4dwNZxsi+cho\n\
                                        zqWOciXrbzifBghBp0Upe5fgR2JRpyB6sMVXIHKkeP9YBQUARm1ECdbdJmPSiNYP\n\
                                        gMKpTaEObMahAAAAgCtugmDSAwIPibrD9MAbJB6KbN15heA6vTtCLOvFe1Hikw94\n\
                                        DYAJz+vlKadbOZW5SfGAOuIe7IynafthWm4RcbXEXxhnVtqHxzMHOZo/Mnoh+bUO\n\
                                        esDSoERyNHokpNK6m1NKbmQeFj4n7rkcrR8hrwX8+Ng8CsBEglDi+ULtVivbAAAA\n\
                                        gQD1vEPRUu9aD7CjkYgDyD2vNRRevARf01ImgT1tpiEA+GLHJ0xMetd7OH0wutAZ\n\
                                        uH26V19Kt4sWpsTwfdl2fIw7XHPc+G1OSqiOk6AS9qT/sy/VL1Wn7CqyAN2jikzn\n\
                                        quE6MbebTUJQSNHK9vQhn+u4hUDdEoMOLTYdWxxcjdJirQAAAIEA0VsOxBRDSTLc\n\
                                        Ar0Y97oCmb/6tU9XGAZwL2E14GVK85PnJNwHrx4aqb0qATE4iPLfE7ms+eBtT8Uj\n\
                                        HF0fxM3KDQiFSrvtgM4JjGTDS4dTYIBD/eQ0/aTaRgLOQqplyBgYVr3x7ATfcIP5\n\
                                        961TfdiJ/QESutdb1KQquFXIMRII4vcAAAAPdGVzdDJAcGlja3kuY29tAQIDBA==\n\
                                        -----END OPENSSH PRIVATE KEY-----";

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(ssh_private_key_pem, None).unwrap();

        let kdf = Kdf::default();

        assert_eq!("test2@picky.com".to_owned(), private_key.comment);
        assert_eq!(kdf, private_key.kdf);
        assert_eq!("none", private_key.cipher_name);
    }

    #[test]
    fn decode_without_passphrase_4096() {
        // ssh-keygen -t rsa -b 4096 -C "test@picky.com" (without the passphrase)
        let ssh_private_key_pem = "-----BEGIN OPENSSH PRIVATE KEY-----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-----END OPENSSH PRIVATE KEY-----";

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(ssh_private_key_pem, None).unwrap();
        let kdf = Kdf::default();
        assert_eq!("test@picky.com".to_owned(), private_key.comment);
        assert_eq!(kdf, private_key.kdf);
        assert_eq!("none", private_key.cipher_name);
    }

    #[test]
    fn decode_with_passphrase_2048() {
        // ssh-keygen -t rsa -b 2048 -C "test_with_pass2@picky.com"
        let passphrase = Some("123123".to_string());
        let ssh_private_key_pem = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                                        b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABBI\n\
                                        MsVovOqXSrZa+iEvQwXzAAAAEAAAAAEAAAEXAAAAB3NzaC1yc2EAAAADAQABAAAB\n\
                                        AQCkR5WaC3NTPZdj9X/bX88YYbR2k5r3aE+I/ezxzbG6xIJi+So9AohypAhReyW9\n\
                                        7XSGut5n6a9O+n/c9nCiXFVoyXbMSdM90Av5bu799+V4w3kBlRzN5D3A6uIZRjgl\n\
                                        wc3Xso9kthneNByB7OjZuSDdmuWE3YOgmW0TirP3dztbtVScLPZUSsEveIMt90aw\n\
                                        uOFWaEUshqb7l713bdEB0Tb77Z1wZpt6UmIgpraV58kN/ahepbY8lirMS4ym75wt\n\
                                        Be6PgyKGKIR3aNQdbfHYMHCgxNQMAFUt2yD9f+JE5HWG7kyKDcLTHCY60dtKTNfp\n\
                                        cByi4Bwm3209V4rGYSKAzFXvAAAD0CdYEpFE8Dda2GBNy1l5vDNdbyJvx7SSP49l\n\
                                        4OmHsgRE2WneNC9CfO2IxPRNXsPEmXimeubqm6alsmJ1Ch+KsdjvyU7WIEnjuonC\n\
                                        lLWx6rhsuppJqZSICbikMUXjhlHpLirGnL0WoaBnLYEVYgu8cMbIgE9BNho+bS+1\n\
                                        qvyIrIdIblJwwc66CJKUYPz1yRA84WIMZOWlsYfeHnCvTGjiYUG2YFayVAuXvAz/\n\
                                        ND3bQYUlO34XOOsJvZxfQNEg1/tzhB7RvcGOG1InoQxT6dZtTp85CkTU/QQ6w2eY\n\
                                        j4qDDmsFm/eSDgEFfOJDLrfHsB4+G2aBZLmgk2bn7vo3JBkcPAETX6kKd7bkyEfh\n\
                                        LVph9i48vbmNJ8mXWiXMoRXqRgkKqBAMVnuXtbKVDVzzlZXIFu1cbuKyt0zUg7jB\n\
                                        IeIdG+5U0L6qygTjOKU6aP+dK1wRc0XyC8jxTJupt2eTEKBLzy4TwlLH5QhEcj1c\n\
                                        coV97PyslJ/NnQx8IKflHxxxQF4CbYgyXt9fWZpBfaD9TVWgsFoKrlZ9HOb6s5WJ\n\
                                        MwijgNLfllKNkJB/KpQUIwMAqEjkfk4HyKeC9sfCHkjkXoZO28GypRR8Bd5M+/Qf\n\
                                        otFvcdRHqbvv+mj1y6nBIv0hv5eqJEil5s/dwGI7cexMGBjPVOPK63kbh6JlMcrb\n\
                                        58jKid1VTzUbxxKm6YfL2aQpGp/veGPZRkm+x3DHoANYLYJ64WRQgBOGcf4QSqiT\n\
                                        xP9Y5ZxfQuheDzOkiQCt3ToTWwguXtVLm3AAUKxUhHVgMy2PQNFXcNsPWGCzhOW1\n\
                                        FzC82iZhuQi7SlTX7iA40np23nMkHu37hkHpfpipySxEIIjv1T0UglqN25hPlHDI\n\
                                        jrTRwcBVxikVhP0IFbDUtlmqSP5MkDEE2ZKTeD0ivd8c2WLO5RUoEICaTVHOx+Mx\n\
                                        OJ9L07ZhA2NMKiMMqhe0bXwZoFFHMUxXh8+iTTy89oE1PQ7xz/d6hJUtbqJ/N2xp\n\
                                        cWMNtnvbjWpxzwhjPGiqKx8GCtpGoAjpUeNqWL9V0a20rJBYqzJGLYfKDd+PW2XT\n\
                                        tOHbQwl0DFNq41jP4nYnaFo2YCjWb3mleRUWkU5SoUHq+vUvs4dxqKjlzvKnK5pc\n\
                                        yH9bnpKPaBI28QHtye7o25AfkOj7eHVSe5CV4u8okVaBEq1OFhBeWm+jx1fBrk82\n\
                                        hEGamuq1GZsZre2y9jauusOFcMXrV5oxJjBLLbGCi0i5ES0O+kBOlB/kY3hdkReC\n\
                                        HCJlMN7v92mkSsadahzwx3fTQWCwgVDg6LLN+xCPGFTMts4XDwg=\n\
                                        -----END OPENSSH PRIVATE KEY-----";

        let private_key: SshPrivateKey = SshPrivateKey::from_pem_str(ssh_private_key_pem, passphrase).unwrap();

        assert_eq!("test_with_pass2@picky.com".to_owned(), private_key.comment);
        assert_eq!(
            Kdf {
                name: "bcrypt".to_owned(),
                option: KdfOption {
                    salt: vec![72, 50, 197, 104, 188, 234, 151, 74, 182, 90, 250, 33, 47, 67, 5, 243],
                    rounds: 16,
                }
            },
            private_key.kdf
        );
        assert_eq!("aes256-ctr", private_key.cipher_name);
    }

    #[test]
    fn encode_without_passphrase_2048() {
        // ssh-keygen -t rsa -b 2048 -C "test2@picky.com" (without the passphrase)
        let ssh_private_key_pem = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                                        b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAABFwAAAAdz\n\
                                        c2gtcnNhAAAAAwEAAQAAAQEAyPYbdoNqjj4EhuYblWIxVKLsmsOff+kLkKlFRsIJ\n\
                                        yE5YUWzPm5LyUH3LoqnL/rw/f/Og37oJO/bEn4P2lSvlf6ZagAGaLo8/8ACw4xKY\n\
                                        UsQFHAEfreIthd/T2u9TEnN+yPS99M99bXG2tV+6He4c61TJfYrq5DsgQuMXCFmt\n\
                                        R/IdJg8qF8lj06qEzjQ1HvXQdXruhm4sQn1HMb3VbdKQFSU3TpmzVysEaOVl3zK7\n\
                                        KirBU9gHIOFZuE3y0oUklFuK6jOhjgQnxeo58Rb00g3p7R+YcpI1i95TAoIQ/tYS\n\
                                        cjnZzByQv+ak1BjgfOjMbEeEQl6kvi2axqTEnFcg0IHu6wAAA8iqDGUDqgxlAwAA\n\
                                        AAdzc2gtcnNhAAABAQDI9ht2g2qOPgSG5huVYjFUouyaw59/6QuQqUVGwgnITlhR\n\
                                        bM+bkvJQfcuiqcv+vD9/86Dfugk79sSfg/aVK+V/plqAAZoujz/wALDjEphSxAUc\n\
                                        AR+t4i2F39Pa71MSc37I9L30z31tcba1X7od7hzrVMl9iurkOyBC4xcIWa1H8h0m\n\
                                        DyoXyWPTqoTONDUe9dB1eu6GbixCfUcxvdVt0pAVJTdOmbNXKwRo5WXfMrsqKsFT\n\
                                        2Acg4Vm4TfLShSSUW4rqM6GOBCfF6jnxFvTSDentH5hykjWL3lMCghD+1hJyOdnM\n\
                                        HJC/5qTUGOB86MxsR4RCXqS+LZrGpMScVyDQge7rAAAAAwEAAQAAAQATZEw6H2xE\n\
                                        1Y8yRTocLCF+fUo/lOjrOt22096veUHgZk73bHyMEp33Tmw8Ag6BQkEOY7/+VsFV\n\
                                        W/aVPfKpalb2/mJ1P7JVE9Wjny1ye/Te57NmhGU+LjkeVf7nfXiSqzpswdEisnL0\n\
                                        AKkUz2vyP2vi+YeH6cPIyjvOuIMcdyrVakejnGbss19ZoXw660X/7TRqG/41KhTm\n\
                                        lkN610JBKI2Rozecx9l3LZ3CTRpOOJ2sfssegvL+qxvvH1YVkRat4dwNZxsi+cho\n\
                                        zqWOciXrbzifBghBp0Upe5fgR2JRpyB6sMVXIHKkeP9YBQUARm1ECdbdJmPSiNYP\n\
                                        gMKpTaEObMahAAAAgCtugmDSAwIPibrD9MAbJB6KbN15heA6vTtCLOvFe1Hikw94\n\
                                        DYAJz+vlKadbOZW5SfGAOuIe7IynafthWm4RcbXEXxhnVtqHxzMHOZo/Mnoh+bUO\n\
                                        esDSoERyNHokpNK6m1NKbmQeFj4n7rkcrR8hrwX8+Ng8CsBEglDi+ULtVivbAAAA\n\
                                        gQD1vEPRUu9aD7CjkYgDyD2vNRRevARf01ImgT1tpiEA+GLHJ0xMetd7OH0wutAZ\n\
                                        uH26V19Kt4sWpsTwfdl2fIw7XHPc+G1OSqiOk6AS9qT/sy/VL1Wn7CqyAN2jikzn\n\
                                        quE6MbebTUJQSNHK9vQhn+u4hUDdEoMOLTYdWxxcjdJirQAAAIEA0VsOxBRDSTLc\n\
                                        Ar0Y97oCmb/6tU9XGAZwL2E14GVK85PnJNwHrx4aqb0qATE4iPLfE7ms+eBtT8Uj\n\
                                        HF0fxM3KDQiFSrvtgM4JjGTDS4dTYIBD/eQ0/aTaRgLOQqplyBgYVr3x7ATfcIP5\n\
                                        961TfdiJ/QESutdb1KQquFXIMRII4vcAAAAPdGVzdDJAcGlja3kuY29tAQIDBA==\n\
                                        -----END OPENSSH PRIVATE KEY-----\x0A";

        let private_key = SshPrivateKey::from_pem_str(ssh_private_key_pem, None).unwrap();
        let ssh_private_key_after = private_key.to_string().unwrap();

        pretty_assertions::assert_eq!(ssh_private_key_pem, ssh_private_key_after.as_str());
    }

    #[test]
    fn encode_with_passphrase_2048() {
        // ssh-keygen -t rsa -b 2048 -C "test_with_pass2@picky.com"
        let passphrase = Some("123123".to_string());
        let ssh_private_key_pem = "-----BEGIN OPENSSH PRIVATE KEY-----\n\
                                        b3BlbnNzaC1rZXktdjEAAAAACmFlczI1Ni1jdHIAAAAGYmNyeXB0AAAAGAAAABBI\n\
                                        MsVovOqXSrZa+iEvQwXzAAAAEAAAAAEAAAEXAAAAB3NzaC1yc2EAAAADAQABAAAB\n\
                                        AQCkR5WaC3NTPZdj9X/bX88YYbR2k5r3aE+I/ezxzbG6xIJi+So9AohypAhReyW9\n\
                                        7XSGut5n6a9O+n/c9nCiXFVoyXbMSdM90Av5bu799+V4w3kBlRzN5D3A6uIZRjgl\n\
                                        wc3Xso9kthneNByB7OjZuSDdmuWE3YOgmW0TirP3dztbtVScLPZUSsEveIMt90aw\n\
                                        uOFWaEUshqb7l713bdEB0Tb77Z1wZpt6UmIgpraV58kN/ahepbY8lirMS4ym75wt\n\
                                        Be6PgyKGKIR3aNQdbfHYMHCgxNQMAFUt2yD9f+JE5HWG7kyKDcLTHCY60dtKTNfp\n\
                                        cByi4Bwm3209V4rGYSKAzFXvAAAD0CdYEpFE8Dda2GBNy1l5vDNdbyJvx7SSP49l\n\
                                        4OmHsgRE2WneNC9CfO2IxPRNXsPEmXimeubqm6alsmJ1Ch+KsdjvyU7WIEnjuonC\n\
                                        lLWx6rhsuppJqZSICbikMUXjhlHpLirGnL0WoaBnLYEVYgu8cMbIgE9BNho+bS+1\n\
                                        qvyIrIdIblJwwc66CJKUYPz1yRA84WIMZOWlsYfeHnCvTGjiYUG2YFayVAuXvAz/\n\
                                        ND3bQYUlO34XOOsJvZxfQNEg1/tzhB7RvcGOG1InoQxT6dZtTp85CkTU/QQ6w2eY\n\
                                        j4qDDmsFm/eSDgEFfOJDLrfHsB4+G2aBZLmgk2bn7vo3JBkcPAETX6kKd7bkyEfh\n\
                                        LVph9i48vbmNJ8mXWiXMoRXqRgkKqBAMVnuXtbKVDVzzlZXIFu1cbuKyt0zUg7jB\n\
                                        IeIdG+5U0L6qygTjOKU6aP+dK1wRc0XyC8jxTJupt2eTEKBLzy4TwlLH5QhEcj1c\n\
                                        coV97PyslJ/NnQx8IKflHxxxQF4CbYgyXt9fWZpBfaD9TVWgsFoKrlZ9HOb6s5WJ\n\
                                        MwijgNLfllKNkJB/KpQUIwMAqEjkfk4HyKeC9sfCHkjkXoZO28GypRR8Bd5M+/Qf\n\
                                        otFvcdRHqbvv+mj1y6nBIv0hv5eqJEil5s/dwGI7cexMGBjPVOPK63kbh6JlMcrb\n\
                                        58jKid1VTzUbxxKm6YfL2aQpGp/veGPZRkm+x3DHoANYLYJ64WRQgBOGcf4QSqiT\n\
                                        xP9Y5ZxfQuheDzOkiQCt3ToTWwguXtVLm3AAUKxUhHVgMy2PQNFXcNsPWGCzhOW1\n\
                                        FzC82iZhuQi7SlTX7iA40np23nMkHu37hkHpfpipySxEIIjv1T0UglqN25hPlHDI\n\
                                        jrTRwcBVxikVhP0IFbDUtlmqSP5MkDEE2ZKTeD0ivd8c2WLO5RUoEICaTVHOx+Mx\n\
                                        OJ9L07ZhA2NMKiMMqhe0bXwZoFFHMUxXh8+iTTy89oE1PQ7xz/d6hJUtbqJ/N2xp\n\
                                        cWMNtnvbjWpxzwhjPGiqKx8GCtpGoAjpUeNqWL9V0a20rJBYqzJGLYfKDd+PW2XT\n\
                                        tOHbQwl0DFNq41jP4nYnaFo2YCjWb3mleRUWkU5SoUHq+vUvs4dxqKjlzvKnK5pc\n\
                                        yH9bnpKPaBI28QHtye7o25AfkOj7eHVSe5CV4u8okVaBEq1OFhBeWm+jx1fBrk82\n\
                                        hEGamuq1GZsZre2y9jauusOFcMXrV5oxJjBLLbGCi0i5ES0O+kBOlB/kY3hdkReC\n\
                                        HCJlMN7v92mkSsadahzwx3fTQWCwgVDg6LLN+xCPGFTMts4XDwg=\n\
                                        -----END OPENSSH PRIVATE KEY-----\x0A";

        let private_key = SshPrivateKey::from_pem_str(ssh_private_key_pem, passphrase).unwrap();
        let ssh_private_key_after = private_key.to_string().unwrap();

        pretty_assertions::assert_eq!(ssh_private_key_pem, ssh_private_key_after.as_str());
    }

    #[test]
    fn test_private_key_generation() {
        let private_key = SshPrivateKey::generate_rsa(2048, Option::Some("123".to_string()), None).unwrap();
        let data = private_key.to_pem().unwrap();
        let _: SshPrivateKey = SshPrivateKey::from_pem(&data, Option::Some("123".to_string())).unwrap();
    }

    #[test]
    fn kdf_option_decode() {
        let mut cursor = Cursor::new(vec![
            0, 0, 0, 24, 0, 0, 0, 16, 72, 50, 197, 104, 188, 234, 151, 74, 182, 90, 250, 33, 47, 67, 5, 243, 0, 0, 0,
            16,
        ]);
        let kdf_option: KdfOption = SshComplexTypeDecode::decode(&mut cursor).unwrap();
        let KdfOption { salt, rounds } = kdf_option;

        assert_eq!(
            vec![72, 50, 197, 104, 188, 234, 151, 74, 182, 90, 250, 33, 47, 67, 5, 243],
            salt
        );
        assert_eq!(16, rounds);

        let mut cursor = Cursor::new(vec![0, 0, 0, 0]);
        let kdf_option: KdfOption = SshComplexTypeDecode::decode(&mut cursor).unwrap();
        let KdfOption { salt, rounds } = kdf_option;

        assert!(salt.is_empty());
        assert_eq!(0, rounds);
    }

    #[test]
    fn kdf_option_encode() {
        let mut res: Vec<u8> = Vec::new();
        let kdf_option = KdfOption {
            salt: vec![72, 50, 197, 104, 188, 234, 151, 74, 182, 90, 250, 33, 47, 67, 5, 243],
            rounds: 16,
        };

        kdf_option.encode(&mut res).unwrap();

        assert_eq!(
            vec![
                0, 0, 0, 24, 0, 0, 0, 16, 72, 50, 197, 104, 188, 234, 151, 74, 182, 90, 250, 33, 47, 67, 5, 243, 0, 0,
                0, 16
            ],
            res
        );

        res.clear();
        let kdf_option = KdfOption::default();
        kdf_option.encode(&mut res).unwrap();

        assert_eq!(vec![0, 0, 0, 0], res);
    }
}
