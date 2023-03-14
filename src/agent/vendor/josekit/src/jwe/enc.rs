pub mod aescbc_hmac;
pub mod aesgcm;

use crate::jwe::enc::aescbc_hmac::AescbcHmacJweEncryption;
pub use AescbcHmacJweEncryption::A128cbcHs256 as A128CBC_HS256;
pub use AescbcHmacJweEncryption::A192cbcHs384 as A192CBC_HS384;
pub use AescbcHmacJweEncryption::A256cbcHs512 as A256CBC_HS512;

use crate::jwe::enc::aesgcm::AesgcmJweEncryption;
pub use AesgcmJweEncryption::A128gcm as A128GCM;
pub use AesgcmJweEncryption::A192gcm as A192GCM;
pub use AesgcmJweEncryption::A256gcm as A256GCM;
