pub mod cmsversion;
pub mod content_info;
pub mod crls;
#[cfg(feature = "ctl")]
pub mod ctl;
pub mod signed_data;
pub mod signer_info;
pub mod timestamp;

use crate::oids;
use signed_data::SignedData;

use picky_asn1::wrapper::{ExplicitContextTag0, ObjectIdentifierAsn1};
use serde::{de, Serialize};

#[derive(Serialize, Debug, PartialEq, Clone)]
pub struct Pkcs7Certificate {
    pub oid: ObjectIdentifierAsn1,
    pub signed_data: ExplicitContextTag0<SignedData>,
}

impl<'de> de::Deserialize<'de> for Pkcs7Certificate {
    fn deserialize<D>(deserializer: D) -> Result<Self, <D as de::Deserializer<'de>>::Error>
    where
        D: de::Deserializer<'de>,
    {
        use std::fmt;

        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = Pkcs7Certificate;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid DER-encoded pcks7 certificate")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let oid: ObjectIdentifierAsn1 =
                    seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;

                let signed_data: ExplicitContextTag0<SignedData> = match Into::<String>::into(&oid.0).as_str() {
                    oids::SIGNED_DATA => seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?,
                    _ => {
                        return Err(serde_invalid_value!(
                            Pkcs7Certificate,
                            "unknown oid type",
                            "SignedData oid"
                        ))
                    }
                };

                Ok(Pkcs7Certificate { oid, signed_data })
            }
        }

        deserializer.deserialize_seq(Visitor)
    }
}

#[cfg(test)]
mod tests {
    use super::Pkcs7Certificate;
    use crate::pkcs7::signer_info::UnsignedAttributeValue;
    use crate::{Attribute, AttributeValues};
    use picky_asn1::wrapper::{Asn1SetOf, OctetStringAsn1};

    #[test]
    fn full_pkcs7_decode() {
        let decoded = base64::decode(
            "MIIF1AYJKoZIhvcNAQcCoIIFxTCCBcECAQExADALBgkqhkiG9w0BBwGgggWnMIIF\
                ozCCA4ugAwIBAgIUXp0J4CZ0ZSsXBh952AQo0XUudpwwDQYJKoZIhvcNAQELBQAw\
                YTELMAkGA1UEBhMCQVIxCzAJBgNVBAgMAkFSMQswCQYDVQQHDAJBUjELMAkGA1UE\
                CgwCQVIxCzAJBgNVBAsMAkFSMQswCQYDVQQDDAJBUjERMA8GCSqGSIb3DQEJARYC\
                QVIwHhcNMjEwNDE5MTEzMzEzWhcNMjIwNDE5MTEzMzEzWjBhMQswCQYDVQQGEwJB\
                UjELMAkGA1UECAwCQVIxCzAJBgNVBAcMAkFSMQswCQYDVQQKDAJBUjELMAkGA1UE\
                CwwCQVIxCzAJBgNVBAMMAkFSMREwDwYJKoZIhvcNAQkBFgJBUjCCAiIwDQYJKoZI\
                hvcNAQEBBQADggIPADCCAgoCggIBAK96+zZ3Ik9K9yqHCz5uTMLAAEKCGo7IjBzc\
                KDY4DlhfSJ1N6MC2US/QBCpLQprLVw0JToMgBt0ErHhLzuACXnpZk6lPqaXruv5+\
                h6bRU3nVcEgkinShGTtybEDHCjbRBODg5aMbsb4vFSzVdk7PijqlUXVn1/1ke3KZ\
                GGYQ/EKweqElpOkjnrawP98gQqVS2wJO++B5DmaEYbe3ketnfv/pPNyQzehyjrsf\
                3jO/5hsRRxHwc6IgsWajRxU12bx3fBqs5CWqe4sZCfJIfpNLkDeE5cvz36P9BLr3\
                s4nKGdDAhMUOYZh6pqX9uZoq3Hm5a76HglY0SpmqOYums97dVcVMxbkMCPPawd+q\
                jz4izc4kEWhDMal3rKr8QqaoFS6ez2hUsUoJW9fPfgiLAArfXLvpWRZCuEGWjWAq\
                US/Kzfc3SvOI4095++IvLxTuvTw+iaEi0J4BLzFBnZs8lUBENJI+zYnhwEhwU8/V\
                vSpjWmvly0RtbCiImFtYpd0y2/TlaJ4jupdR9//8gb21W/nKNzRrlYzVhYfdM+BP\
                itNzeHKQHNzfy18LHBRpvqlp/nV3KhxTuEe/CvIsFS5xRtTmUICBwBC4ycq8cV/6\
                Ek4FQTCYo08VQ9F68qX9iVAk+ajaRr3cE6+oX+aXIRx6GZ2KkC/NWcLnjOPy2flR\
                y1lBxUmfAgMBAAGjUzBRMB0GA1UdDgQWBBRxxrWG5tXUbtOytggfIuTu/sP/4TAf\
                BgNVHSMEGDAWgBRxxrWG5tXUbtOytggfIuTu/sP/4TAPBgNVHRMBAf8EBTADAQH/\
                MA0GCSqGSIb3DQEBCwUAA4ICAQBZ5AyGS/U5Fqo5MpMvTlIuEXKDL4tTFyFXvBg6\
                iowN9NylLeH2xyD5Deb2EOjoAJTnrhtCNmyOh3JmPpyFMN2S0LdzGeQS5idHlCsT\
                PZpj0qJp+iexDS0syrYX/9/+L+5cR1LBVXEoK3kF0ZpSHUOxZkIx11zHjKohs+FF\
                Hanhmx7cyVdDhBwtBcQfR41al6idt155xyhuYVTyXi0XSKr3YgBWXzjKnfrxsEe0\
                7Zo18ZtMe0p42yYwEhQaPL0UQkuSC7hAilOO3YWQ51Vnk3QJ7kw+EEqed6LNuAsS\
                Qt8h4F7fiVuO4UG5CToDwK9bEw4zfQ8Xrm+6rwy/3CWC8L/YZ8Paj89+2JB3woIv\
                F+6QvKTPpQ0arM4dI82nsyHSdt2bxXkLJ7iZ/D1vJZkWzBrpvuTmeHAKiFIj2hfJ\
                5FruZrC/60BKrbx4FAniXinNSzk43l4Q42JD6zGkex7mxXURkxqbbz6TAqSmbwgp\
                ygxNWPIKIvldXq1yeNt9lPfHANqd+dCrpnkNCXVwaFbDqTaltYdaB4zg9HlzvlBK\
                Eh49eilHGchkyMBawo80ctWy9UNH/Yt3uiwuga0Q2sCLlrbPxE5Ro3Ou/SZF9YtZ\
                Ee/Xdsl0pUmdAylBzp08AJWCuPheE7XjrnfDlPz4BRuiB+qOMDO/ngLNZ0rFbiIV\
                3ojRzKEAMQA=",
        )
        .unwrap();

        let pkcs7: Pkcs7Certificate = picky_asn1_der::from_bytes(decoded.as_slice()).unwrap();
        check_serde!(pkcs7: Pkcs7Certificate in decoded);
    }

    #[test]
    fn full_pkcs7_decode_with_ca_root() {
        let decoded = base64::decode(
            "MIIIwgYJKoZIhvcNAQcCoIIIszCCCK8CAQExADALBgkqhkiG9w0BBwGgggiVMIIE\
                nDCCAoQCFGe148Dqygm4BCpH70eVHP64Kf3zMA0GCSqGSIb3DQEBCwUAMIGHMQsw\
                CQYDVQQGEwJUWTELMAkGA1UECAwCVFkxETAPBgNVBAcMCFNvbWVDaXR5MRQwEgYD\
                VQQKDAtTb21lQ29tcGFueTERMA8GA1UECwwIU29tZVVuaXQxDzANBgNVBAMMBk15\
                TmFtZTEeMBwGCSqGSIb3DQEJARYPbXltYWlsQG1haWwuY29tMB4XDTIxMDQxOTEy\
                NDgwMloXDTI0MDExNDEyNDgwMlowgYwxCzAJBgNVBAYTAlRZMQswCQYDVQQIDAJU\
                WTERMA8GA1UEBwwIU29tZUNpdHkxGTAXBgNVBAoMEFNvbWVPcmdhbml6YXRpb24x\
                ETAPBgNVBAsMCFNvbWVVbml0MQ8wDQYDVQQDDAZNeU5hbWUxHjAcBgkqhkiG9w0B\
                CQEWD215bWFpbEBtYWlsLmNvbTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoC\
                ggEBAJ3PyrdEwqN4SBK/1RIkH9s0MSJ+qWLvgPWozvWgOmK1peYWEpcaR/Kph8rq\
                rPrePLXX6ZM7Oyy/rtf1A521hNdPAvNbJ/Dhxou/ivavqoLoc6cMhM/0LFKrN0en\
                BCwfC1XKOF3F+LgkitKtbF9iWGbt4Pp5GtrEjjCdwHzF/5tmsq+yzcnQGTiX24BA\
                pvhlHuXpcLvBEDwNXJ2Tj812hJO1gZ8iOyIKY8BMBygRLp2pE3z/9w1E0gF03Y3C\
                N4ts4VDi/pFstxHRfSX7V7TdLxcm8CsZhmbxYzKOG95TORwY9q2nGQcRKAzyHd5w\
                a25ri/LbuHaz2LnUQrLYMXOHag0CAwEAATANBgkqhkiG9w0BAQsFAAOCAgEAGaKl\
                8Bq5M6BIU8gYXIx1kRE/WOMonUAwn6agoqeRVROiHz0XSkSArUM76WZ98Ndvbbvj\
                4p+yCMeiX12zro0qj1hxxBF6cYxRxutukJjU/OjTCR7jR57QHVVHzjBDW2PQoHIE\
                fX399yr2IILzAeI8xvLx8v2y+o7vfCWW5Sd8x7puww1FGNA/QqKBu/kw4NobybwS\
                j+SC38qfBQQsKghbuJbxpwLuY3yixqwPV8Swlzc1PrAwtA+RWabQDHjShnTBppqu\
                XNTFhGbPDdTHzECnRxg9cQbqSpiOkdnxEpagy8c7ccCwvHjD2SQGr44ydHg5FYPB\
                k2wXKc8EFtmR4kyWg1zYjuboY0/0iaUkyWOZYy6NZE/ktwZKR02gXoN1I3YSsbw/\
                fytr4ldkqq6bkpgMK/60CKiQvI8tdxcnQejeSlJfYqzptIlnsPH8X1x/kfQ8dWFj\
                YIyxvRDe+26/1wOXodgTNwrn02FzNEcxqyOLL9YzYvbq1UiNi2n6CBaAJKdU7NhE\
                dnzb81P4uOfs0QbsWGkVE3T9NzRlJlEPjei+srUZDvFYDjTTo2rTITOPDLPzSJlE\
                UfxVV3uaRHc8Z07oTiaW2H/eizOwwBLbgVRKMy74dk4wC/3P1CSwyd4Z+c/l2LZ5\
                8Z2LMjIFw/eVYCfAiOmS/xyGqYZoXNXVZlp2/UswggPxMIIC2aADAgECAhQY0ZCe\
                SXAknAwNvZQgZvONNLI/xjANBgkqhkiG9w0BAQsFADCBhzELMAkGA1UEBhMCVFkx\
                CzAJBgNVBAgMAlJGMREwDwYDVQQHDAhTb21lQ2l0eTEUMBIGA1UECgwLU29tZUNv\
                bXBhbnkxETAPBgNVBAsMCFNvbWVVbml0MQ8wDQYDVQQDDAZNeU5hbWUxHjAcBgkq\
                hkiG9w0BCQEWD215bWFpbEBtYWlsLmNvbTAeFw0yMTA0MTkxMjI4MzNaFw0yNDA0\
                MDkxMjI4MzNaMIGHMQswCQYDVQQGEwJUWTELMAkGA1UECAwCUkYxETAPBgNVBAcM\
                CFNvbWVDaXR5MRQwEgYDVQQKDAtTb21lQ29tcGFueTERMA8GA1UECwwIU29tZVVu\
                aXQxDzANBgNVBAMMBk15TmFtZTEeMBwGCSqGSIb3DQEJARYPbXltYWlsQG1haWwu\
                Y29tMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA1a+nyoNA6hkFplgN\
                ajApR5lITmhxGJGYvrm3w4fYTqQ+eGr9PxQsEfhQQqy8+nOyCSRL9FJ2YwGdwrPZ\
                KjJc1RIVWpCk+vxbm04C8PMlDiygpeD9l7tfZDTdJD4npRvHlltUSbK69/j0djC+\
                aJr+DMT3h2fU/9mDDfVaKB30Q0mwOdmtLGcOAXddN9AJBVP9b6GekAE7jKC037jK\
                UUrA3h5bw0rvic+jvtKnf1rsh5VYfHelJnxRnZ/XBijy99fZRf260i8gzp0+/OSg\
                39ggjOPlrGcPpLPcHShMlTK553GmO64T7IgBtmH8LdG/XInkcRw0oZ6BK5lUCSPp\
                UQ4TfQIDAQABo1MwUTAdBgNVHQ4EFgQUznGR6rk3Nzi+3z80yteN8IPI3TYwHwYD\
                VR0jBBgwFoAUznGR6rk3Nzi+3z80yteN8IPI3TYwDwYDVR0TAQH/BAUwAwEB/zAN\
                BgkqhkiG9w0BAQsFAAOCAQEAjcK4L3VqsdUHmAyL9VBxBK4msWOzKMStUJ0d9+j6\
                LLMMo39/0RLqvwiREP1JELCzWdKCrMKRtKbQh/e7dQoFR4zWezJ5ChtKRqAlUdVt\
                m7yr61Ua0ftpkJtcb5+b8vP+cruAvnrjpW8VQNOkbce+VjOjl287FdfliZgpTCee\
                5UPxb2USETPoTohJOPpE27w6/1Ztb8AUgDzjZwd50Ty1ci6SBeFsD7YBgnSZO9ze\
                S8zhKeY18ivsny0RKPO28/fO7rhExogn4sg12H6kaM3NokmDUf8FVy/Np0LCFreU\
                cjZ0Bdoi73yZiQcNp8lb1Hm5jJbq2Oa1aY88KZ1Hdyx+jqEAMQA=",
        )
        .unwrap();

        let pkcs7: Pkcs7Certificate = picky_asn1_der::from_bytes(decoded.as_slice()).unwrap();
        check_serde!(pkcs7: Pkcs7Certificate in decoded);
    }

    #[test]
    fn decode_pkcs7_with_timestamp() {
        let decoded = base64::decode(
            "MIIkWwYJKoZIhvcNAQcCoIIkTDCCJEgCAQExDzANBglghkgBZQMEAgEFADB5Bgor\
                  BgEEAYI3AgEEoGswaTA0BgorBgEEAYI3AgEeMCYCAwEAAAQQH8w7YFlLCE63JNLG\
                  KX7zUQIBAAIBAAIBAAIBAAIBADAxMA0GCWCGSAFlAwQCAQUABCD+xe8u4YoS6UEO\
                  jtW70wceL89huvuluOvdcbeefpOXLqCCDYEwggX/MIID56ADAgECAhMzAAABh3IX\
                  chVZQMcJAAAAAAGHMA0GCSqGSIb3DQEBCwUAMH4xCzAJBgNVBAYTAlVTMRMwEQYD\
                  VQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNy\
                  b3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMTH01pY3Jvc29mdCBDb2RlIFNpZ25p\
                  bmcgUENBIDIwMTEwHhcNMjAwMzA0MTgzOTQ3WhcNMjEwMzAzMTgzOTQ3WjB0MQsw\
                  CQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9u\
                  ZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMR4wHAYDVQQDExVNaWNy\
                  b3NvZnQgQ29ycG9yYXRpb24wggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIB\
                  AQDOt8kLc7P3T7MKIhouYHewMFmnq8Ayu7FOhZCQabVwBp2VS4WyB2Qe4TQBT8aB\
                  znANDEPjHKNdPT8Xz5cNali6XHefS8i/WXtF0vSsP8NEv6mBHuA2p1fw2wB/F0dH\
                  sJ3GfZ5c0sPJjklsiYqPw59xJ54kM91IOgiO2OUzjNAljPibjCWfH7UzQ1TPHc4d\
                  weils8GEIrbBRb7IWwiObL12jWT4Yh71NQgvJ9Fn6+UhD9x2uk3dLj84vwt1NuFQ\
                  itKJxIV0fVsRNR3abQVOLqpDugbr0SzNL6o8xzOHL5OXiGGwg6ekiXA1/2XXY7yV\
                  Fc39tledDtZjSjNbex1zzwSXAgMBAAGjggF+MIIBejAfBgNVHSUEGDAWBgorBgEE\
                  AYI3TAgBBggrBgEFBQcDAzAdBgNVHQ4EFgQUhov4ZyO96axkJdMjpzu2zVXOJcsw\
                  UAYDVR0RBEkwR6RFMEMxKTAnBgNVBAsTIE1pY3Jvc29mdCBPcGVyYXRpb25zIFB1\
                  ZXJ0byBSaWNvMRYwFAYDVQQFEw0yMzAwMTIrNDU4Mzg1MB8GA1UdIwQYMBaAFEhu\
                  ZOVQBdOCqhc3NyK1bajKdQKVMFQGA1UdHwRNMEswSaBHoEWGQ2h0dHA6Ly93d3cu\
                  bWljcm9zb2Z0LmNvbS9wa2lvcHMvY3JsL01pY0NvZFNpZ1BDQTIwMTFfMjAxMS0w\
                  Ny0wOC5jcmwwYQYIKwYBBQUHAQEEVTBTMFEGCCsGAQUFBzAChkVodHRwOi8vd3d3\
                  Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2NlcnRzL01pY0NvZFNpZ1BDQTIwMTFfMjAx\
                  MS0wNy0wOC5jcnQwDAYDVR0TAQH/BAIwADANBgkqhkiG9w0BAQsFAAOCAgEAixmy\
                  S6E6vprWD9KFNIB9G5zyMuIjZAOuUJ1EK/Vlg6Fb3ZHXjjUwATKIcXbFuFC6Wr4K\
                  NrU4DY/sBVqmab5AC/je3bpUpjtxpEyqUqtPc30wEg/rO9vmKmqKoLPT37svc2NV\
                  BmGNl+85qO4fV/w7Cx7J0Bbqk19KcRNdjt6eKoTnTPHBHlVHQIHZpMxacbFOAkJr\
                  qAVkYZdz7ikNXTxV+GRb36tC4ByMNxE2DF7vFdvaiZP0CVZ5ByJ2gAhXMdK9+usx\
                  zVk913qKde1OAuWdv+rndqkAIm8fUlRnr4saSCg7cIbUwCCf116wUJ7EuJDg0vHe\
                  yhnCeHnBbyH3RZkHEi2ofmfgnFISJZDdMAeVZGVOh20Jp50XBzqokpPzeZ6zc1/g\
                  yILNyiVgE+RPkjnUQshd1f1PMgn3tns2Cz7bJiVUaqEO3n9qRFgy5JuLae6UweGf\
                  AeOo3dgLZxikKzYs3hDMaEtJq8IP71cX7QXe6lnMmXU/Hdfz2p897Zd+kU+vZvKI\
                  3cwLfuVQgK2RZ2z+Kc3K3dRPz2rXycK5XCuRZmvGab/WbrZiC7wJQapgBodltMI5\
                  GMdFrBg9IeF7/rP4EqVQXeKtevTlZXjpuNhhjuR+2DMt/dWufjXpiW91bo3aH6Ea\
                  jOALXmoxgltCp1K7hrS6gmsvj94cLRf50QQ4U8Qwggd6MIIFYqADAgECAgphDpDS\
                  AAAAAAADMA0GCSqGSIb3DQEBCwUAMIGIMQswCQYDVQQGEwJVUzETMBEGA1UECBMK\
                  V2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0\
                  IENvcnBvcmF0aW9uMTIwMAYDVQQDEylNaWNyb3NvZnQgUm9vdCBDZXJ0aWZpY2F0\
                  ZSBBdXRob3JpdHkgMjAxMTAeFw0xMTA3MDgyMDU5MDlaFw0yNjA3MDgyMTA5MDla\
                  MH4xCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdS\
                  ZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMT\
                  H01pY3Jvc29mdCBDb2RlIFNpZ25pbmcgUENBIDIwMTEwggIiMA0GCSqGSIb3DQEB\
                  AQUAA4ICDwAwggIKAoICAQCr8PpyEBwurdhuqoIQTTS68rZYIZ9CGypr6VpQqrgG\
                  OBoESbp/wwwe3TdrxhLYC/A4wpkGsMg51QEUMULTiQ15ZId+lGAkbK+eSZzpaF7S\
                  35tTsgosw6/ZqSuuegmv15ZZymAaBelmdugyUiYSL+erCFDPs0S3XdjELgN1q2jz\
                  y23zOlyhFvRGuuA4ZKxuZDV4pqBjDy3TQJP4494HDdVceaVJKecNvqATd76UPe/7\
                  4ytaEB9NViiienLgEjq3SV7Y7e1DkYPZe7J7hhvZPrGMXeiJT4Qa8qEvWeSQOy2u\
                  M1jFtz7+MtOzAz2xsq+SOH7SnYAs9U5WkSE1JcM5bmR/U7qcD60ZI4TL9LoDho33\
                  X/DQUr+MlIe8wCF0JV8YKLbMJyg4JZg5SjbPfLGSrhwjp6lm7GEfauEoSZ1fiOIl\
                  XdMhSz5SxLVXPyQD8NF6Wy/VI+NwXQ9RRnez+ADhvKwCgl/bwBWzvRvUVUvnOaEP\
                  6SNJvBi4RHxF5MHDcnrgcuck379GmcXvwhxX24ON7E1JMKerjt/sW5+v/N2wZuLB\
                  l4F77dbtS+dJKacTKKanfWeA5opieF+yL4TXV5xcv3coKPHtbcMojyyPQDdPweGF\
                  RInECUzF1KVDL3SV9274eCBYLBNdYJWaPk8zhNqwiBfenk70lrC8RqBsmNLg1oiM\
                  CwIDAQABo4IB7TCCAekwEAYJKwYBBAGCNxUBBAMCAQAwHQYDVR0OBBYEFEhuZOVQ\
                  BdOCqhc3NyK1bajKdQKVMBkGCSsGAQQBgjcUAgQMHgoAUwB1AGIAQwBBMAsGA1Ud\
                  DwQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFHItOgIxkEO5FAVO\
                  4eqnxzHRI4k0MFoGA1UdHwRTMFEwT6BNoEuGSWh0dHA6Ly9jcmwubWljcm9zb2Z0\
                  LmNvbS9wa2kvY3JsL3Byb2R1Y3RzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                  Mi5jcmwwXgYIKwYBBQUHAQEEUjBQME4GCCsGAQUFBzAChkJodHRwOi8vd3d3Lm1p\
                  Y3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                  Mi5jcnQwgZ8GA1UdIASBlzCBlDCBkQYJKwYBBAGCNy4DMIGDMD8GCCsGAQUFBwIB\
                  FjNodHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2RvY3MvcHJpbWFyeWNw\
                  cy5odG0wQAYIKwYBBQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AcABvAGwAaQBjAHkA\
                  XwBzAHQAYQB0AGUAbQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAGfyhqWY\
                  4FR5Gi7T2HRnIpsLlhHhY5KZQpZ90nkMkMFlXy4sPvjDctFtg/6+P+gKyju/R6mj\
                  82nbY78iNaWXXWWEkH2LRlBV2AySfNIaSxzzPEKLUtCw/WvjPgcuKZvmPRul1LUd\
                  d5Q54ulkyUQ9eHoj8xN9ppB0g430yyYCRirCihC7pKkFDJvtaPpoLpWgKj8qa1hJ\
                  Yx8JaW5amJbkg/TAj/NGK978O9C9Ne9uJa7lryft0N3zDq+ZKJeYTQ49C/IIidYf\
                  wzIY4vDFLc5bnrRJOQrGCsLGra7lstnbFYhRRVg4MnEnGn+x9Cf43iw6IGmYslmJ\
                  aG5vp7d0w0AFBqYBKig+gj8TTWYLwLNN9eGPfxxvFX1Fp3blQCplo8NdUmKGwx1j\
                  NpeG39rz+PIWoZon4c2ll9DuXWNB41sHnIc+BncG0QaxdR8UvmFhtfDcxhsEvt9B\
                  xw4o7t5lL+yX9qFcltgA1qFGvVnzl6UJS0gQmYAf0AApxbGbpT9Fdx41xtKiop96\
                  eiL6SJUfq/tHI4D1nvi/a7dLl+LrdXga7Oo3mXkYS//WsyNodeav+vyL6wuA6mk7\
                  r/ww7QRMjt/fdW1jkT3RnVZOT7+AVyKheBEyIXrvQQqxP/uozKRdwaGIm1dxVk5I\
                  RcBCyZt2WwqASGv9eZ/BvW1taslScxMNelDNMYIWMDCCFiwCAQEwgZUwfjELMAkG\
                  A1UEBhMCVVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQx\
                  HjAcBgNVBAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEoMCYGA1UEAxMfTWljcm9z\
                  b2Z0IENvZGUgU2lnbmluZyBQQ0EgMjAxMQITMwAAAYdyF3IVWUDHCQAAAAABhzAN\
                  BglghkgBZQMEAgEFAKCBrjAZBgkqhkiG9w0BCQMxDAYKKwYBBAGCNwIBBDAcBgor\
                  BgEEAYI3AgELMQ4wDAYKKwYBBAGCNwIBFTAvBgkqhkiG9w0BCQQxIgQgCZ2K5xbK\
                  27tyibqtMV5AHpyNN7lNy3nCNEZ+gshCPtAwQgYKKwYBBAGCNwIBDDE0MDKgFIAS\
                  AE0AaQBjAHIAbwBzAG8AZgB0oRqAGGh0dHA6Ly93d3cubWljcm9zb2Z0LmNvbTAN\
                  BgkqhkiG9w0BAQEFAASCAQAyeEIPHCeezdvuJlvHLPAx0mBb2+36yOGP5QAIT6x/\
                  eMCsK3QEYylFlfSjDsBWHUFweyharjwu/sw/KDZcbfwz82aaOpkxbuAhPjHRX84S\
                  pqHxNxw7PHq6N6XOoe61YLEa93OF3A9WACsenIrjBssA6IhSr/Q/oVR78MKxh6VH\
                  qGAthEaOm/NKb8c0DmhUiHKtZMy05oNYDMkYcCyehhCBLf9/nehabRuHk8vMf0AF\
                  5WsJQaUvARJRhXJamv1Mr9WVKXhW+nBCmauazkJIagJNuakh1ype4/gNAKTit4H5\
                  MghO2ERzx1XVg1Kvrsu87VdPXBHf+remb0JvXcYQyXndoYITujCCE7YGCisGAQQB\
                  gjcDAwExghOmMIITogYJKoZIhvcNAQcCoIITkzCCE48CAQMxDzANBglghkgBZQME\
                  AgEFADCCAVgGCyqGSIb3DQEJEAEEoIIBRwSCAUMwggE/AgEBBgorBgEEAYRZCgMB\
                  MDEwDQYJYIZIAWUDBAIBBQAEIAMc3Z52CqtJPqjV3aVgo0scv2C5S/V6jiQGZXcy\
                  SupuAgZeoMfc5PYYEzIwMjAwNDI0MDExNzQ0LjI2NlowBwIBAYACAfSggdSkgdEw\
                  gc4xCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdS\
                  ZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsT\
                  IE1pY3Jvc29mdCBPcGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFs\
                  ZXMgVFNTIEVTTjo3MjhELUM0NUYtRjlFQjElMCMGA1UEAxMcTWljcm9zb2Z0IFRp\
                  bWUtU3RhbXAgU2VydmljZaCCDyIwggT1MIID3aADAgECAhMzAAABBAkBdQhYhy0p\
                  AAAAAAEEMA0GCSqGSIb3DQEBCwUAMHwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpX\
                  YXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQg\
                  Q29ycG9yYXRpb24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1lLVN0YW1wIFBDQSAy\
                  MDEwMB4XDTE5MDkwNjIwNDExOFoXDTIwMTIwNDIwNDExOFowgc4xCzAJBgNVBAYT\
                  AlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYD\
                  VQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29mdCBP\
                  cGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVTTjo3\
                  MjhELUM0NUYtRjlFQjElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAgU2Vy\
                  dmljZTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAMgtB6ARuhhmlpTh\
                  YPwWgmtO2oNVTTZyHgYQBc3GH/J1w6bhgTcgpNiZnGZe2kv1Abyg7ABSP6ekgpRh\
                  WpByx5gOeOxpllPXkCxpiMlKFFx++Rnxg0N1YFN2aAsVj9GRMWc3R6hPKtgFMHXU\
                  LPxji3fu6DTgjfOi2pih5r/O+cp1Oi8KvdT+8p5JlROk1/85nsTggE80CudP/Nhu\
                  iIrSvmDNKVmOMF3afUWUswVP6v6t9cGjSWG3GMGNZe8FB3VVOL+pNtCbRV83qhQt\
                  kyIyA8HvGaciAfrXZi/QD5C/vK7XcvoeHbizh7j5lXUD3PiH0ffqHvMp58lsU/Aj\
                  pqr5ZGcCAwEAAaOCARswggEXMB0GA1UdDgQWBBSY1V7fwkQaDhcBi/GZ08MisOia\
                  6jAfBgNVHSMEGDAWgBTVYzpcijGQ80N7fEYbxTNoWoVtVTBWBgNVHR8ETzBNMEug\
                  SaBHhkVodHRwOi8vY3JsLm1pY3Jvc29mdC5jb20vcGtpL2NybC9wcm9kdWN0cy9N\
                  aWNUaW1TdGFQQ0FfMjAxMC0wNy0wMS5jcmwwWgYIKwYBBQUHAQEETjBMMEoGCCsG\
                  AQUFBzAChj5odHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1Rp\
                  bVN0YVBDQV8yMDEwLTA3LTAxLmNydDAMBgNVHRMBAf8EAjAAMBMGA1UdJQQMMAoG\
                  CCsGAQUFBwMIMA0GCSqGSIb3DQEBCwUAA4IBAQA9FdSzd2l8NAMX17RFeWLhOqnO\
                  AgyXIjH8tdW1yA94Zdzyn8NeukcjyIL7/Pkj8R7KEtEUL0cfRnds6KITaPBXxlos\
                  z1i+kMhfd6d4kSgnPWm0qoA14fqxJUM6P5fZfWRGUrtkNJha6N8Id1Ciuyibq7K0\
                  3EnTLgli3EX1LXlzBOyyyjM3hDGVxgPk9D7Bw5ikgVju+Yql+tXjjgG/oFw+WJvw\
                  BN7YunaRV06JKZwsYGPsOYA1qyc8VXBoyeKGFKhI2oThT/P7IM3hCxLNc4fix3sL\
                  aKe4NZta0rjdssY8Kz+Z4sr8T9daXSFa7kUpKVw5277+0QFCc6bkrHjlKB/lMIIG\
                  cTCCBFmgAwIBAgIKYQmBKgAAAAAAAjANBgkqhkiG9w0BAQsFADCBiDELMAkGA1UE\
                  BhMCVVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAc\
                  BgNVBAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEyMDAGA1UEAxMpTWljcm9zb2Z0\
                  IFJvb3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTAwHhcNMTAwNzAxMjEzNjU1\
                  WhcNMjUwNzAxMjE0NjU1WjB8MQswCQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGlu\
                  Z3RvbjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBv\
                  cmF0aW9uMSYwJAYDVQQDEx1NaWNyb3NvZnQgVGltZS1TdGFtcCBQQ0EgMjAxMDCC\
                  ASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAKkdDbx3EYo6IOz8E5f1+n9p\
                  lGt0VBDVpQoAgoX77XxoSyxfxcPlYcJ2tz5mK1vwFVMnBDEfQRsalR3OCROOfGEw\
                  WbEwRA/xYIiEVEMM1024OAizQt2TrNZzMFcmgqNFDdDq9UeBzb8kYDJYYEbyWEeG\
                  MoQedGFnkV+BVLHPk0ySwcSmXdFhE24oxhr5hoC732H8RsEnHSRnEnIaIYqvS2SJ\
                  UGKxXf13Hz3wV3WsvYpCTUBR0Q+cBj5nf/VmwAOWRH7v0Ev9buWayrGo8noqCjHw\
                  2k4GkbaICDXoeByw6ZnNPOcvRLqn9NxkvaQBwSAJk3jN/LzAyURdXhacAQVPIk0C\
                  AwEAAaOCAeYwggHiMBAGCSsGAQQBgjcVAQQDAgEAMB0GA1UdDgQWBBTVYzpcijGQ\
                  80N7fEYbxTNoWoVtVTAZBgkrBgEEAYI3FAIEDB4KAFMAdQBiAEMAQTALBgNVHQ8E\
                  BAMCAYYwDwYDVR0TAQH/BAUwAwEB/zAfBgNVHSMEGDAWgBTV9lbLj+iiXGJo0T2U\
                  kFvXzpoYxDBWBgNVHR8ETzBNMEugSaBHhkVodHRwOi8vY3JsLm1pY3Jvc29mdC5j\
                  b20vcGtpL2NybC9wcm9kdWN0cy9NaWNSb29DZXJBdXRfMjAxMC0wNi0yMy5jcmww\
                  WgYIKwYBBQUHAQEETjBMMEoGCCsGAQUFBzAChj5odHRwOi8vd3d3Lm1pY3Jvc29m\
                  dC5jb20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dF8yMDEwLTA2LTIzLmNydDCBoAYD\
                  VR0gAQH/BIGVMIGSMIGPBgkrBgEEAYI3LgMwgYEwPQYIKwYBBQUHAgEWMWh0dHA6\
                  Ly93d3cubWljcm9zb2Z0LmNvbS9QS0kvZG9jcy9DUFMvZGVmYXVsdC5odG0wQAYI\
                  KwYBBQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AUABvAGwAaQBjAHkAXwBTAHQAYQB0\
                  AGUAbQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAAfmiFEN4sbgmD+BcQM9\
                  naOhIW+z66bM9TG+zwXiqf76V20ZMLPCxWbJat/15/B4vceoniXj+bzta1RXCCtR\
                  gkQS+7lTjMz0YBKKdsxAQEGb3FwX/1z5Xhc1mCRWS3TvQhDIr79/xn/yN31aPxzy\
                  mXlKkVIArzgPF/UveYFl2am1a+THzvbKegBvSzBEJCI8z+0DpZaPWSm8tv0E4XCf\
                  Mkon/VWvL/625Y4zu2JfmttXQOnxzplmkIz/amJ/3cVKC5Em4jnsGUpxY517IW3D\
                  nKOiPPp/fZZqkHimbdLhnPkd/DjYlPTGpQqWhqS9nhquBEKDuLWAmyI4ILUl5WTs\
                  9/S/fmNZJQ96LjlXdqJxqgaKD4kWumGnEcua2A5HmoDF0M2n0O99g/DhO3EJ3110\
                  mCIIYdqwUB5vvfHhAN/nMQekkzr3ZUd46PioSKv33nJ+YWtvd6mBy6cJrDm77MbL\
                  2IK0cs0d9LiFAR6A+xuJKlQ5slvayA1VmXqHczsI5pgt6o3gMy4SKfXAL1QnIffI\
                  rE7aKLixqduWsqdCosnPGUFN4Ib5KpqjEWYw07t0MkvfY3v1mYovG8chr1m1rtxE\
                  PJdQcdeh0sVV42neV8HR3jDA/czmTfsNv11P6Z0eGTgvvM9YBS7vDaBQNdrvCScc\
                  1bN+NR4Iuto229Nfj950iEkSoYIDsDCCApgCAQEwgf6hgdSkgdEwgc4xCzAJBgNV\
                  BAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4w\
                  HAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29m\
                  dCBPcGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVT\
                  Tjo3MjhELUM0NUYtRjlFQjElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAg\
                  U2VydmljZaIlCgEBMAkGBSsOAwIaBQADFQCzRh5/R0jzKEyIVLZzGHgW3BUKfaCB\
                  3jCB26SB2DCB1TELMAkGA1UEBhMCVVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAO\
                  BgNVBAcTB1JlZG1vbmQxHjAcBgNVBAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEp\
                  MCcGA1UECxMgTWljcm9zb2Z0IE9wZXJhdGlvbnMgUHVlcnRvIFJpY28xJzAlBgNV\
                  BAsTHm5DaXBoZXIgTlRTIEVTTjo0REU5LTBDNUUtM0UwOTErMCkGA1UEAxMiTWlj\
                  cm9zb2Z0IFRpbWUgU291cmNlIE1hc3RlciBDbG9jazANBgkqhkiG9w0BAQUFAAIF\
                  AOJMR0MwIhgPMjAyMDA0MjQwMDU2MzVaGA8yMDIwMDQyNTAwNTYzNVowdzA9Bgor\
                  BgEEAYRZCgQBMS8wLTAKAgUA4kxHQwIBADAKAgEAAgIQsAIB/zAHAgEAAgIY9jAK\
                  AgUA4k2YwwIBADA2BgorBgEEAYRZCgQCMSgwJjAMBgorBgEEAYRZCgMBoAowCAIB\
                  AAIDFuNgoQowCAIBAAIDB6EgMA0GCSqGSIb3DQEBBQUAA4IBAQCU62Zx3p4eDKjR\
                  X6SThnb5SUK7xq3KMucFnp+c0dUmGrX2yQLhXkhHloWkKw7mjEdjZrMc98gYZGqA\
                  Y0ziY9vEjIQWvsF9F2xENTDkTmPalXGX4M30fW4HjGpids9Ey5G6un4sOV78hzsD\
                  eI/p41S2KXuWRwBIpG0s/h/eA7Kn7mkWpBDVjVuk4t4xTCTR7z/ms+itrqCOuFcV\
                  HRhoWWkE+OZl/Din7GejZIuo0o1oKcFAIuEOG/WODi0uSZcLFpKLPfiK/BK2nFFv\
                  Gha8r9VK7hXGCCeIFUQ7cRGYAUuYjc5BsGLCaDeq6p6NK9/VwC1r9wmt2wzEdA/h\
                  a6OIafxiMYIC9TCCAvECAQEwgZMwfDELMAkGA1UEBhMCVVMxEzARBgNVBAgTCldh\
                  c2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNVBAoTFU1pY3Jvc29mdCBD\
                  b3Jwb3JhdGlvbjEmMCQGA1UEAxMdTWljcm9zb2Z0IFRpbWUtU3RhbXAgUENBIDIw\
                  MTACEzMAAAEECQF1CFiHLSkAAAAAAQQwDQYJYIZIAWUDBAIBBQCgggEyMBoGCSqG\
                  SIb3DQEJAzENBgsqhkiG9w0BCRABBDAvBgkqhkiG9w0BCQQxIgQgHn2Eb60N7JNv\
                  3A3vUOf5J/ZyYXxiwaHelyJ17ZCZDeYwgeIGCyqGSIb3DQEJEAIMMYHSMIHPMIHM\
                  MIGxBBSzRh5/R0jzKEyIVLZzGHgW3BUKfTCBmDCBgKR+MHwxCzAJBgNVBAYTAlVT\
                  MRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQK\
                  ExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1l\
                  LVN0YW1wIFBDQSAyMDEwAhMzAAABBAkBdQhYhy0pAAAAAAEEMBYEFPnzEPMl8q6k\
                  HOyQFJu9m/ZL40eoMA0GCSqGSIb3DQEBCwUABIIBAIbZuY7fGpCJcVSCjgBYq8ny\
                  ZS18sH7qH0BTCqgcgR8ghDn1NZ4Y6bKDfBQkwfwDWNWVpwAex06skEbw/tdmv70b\
                  nIzcTGjatnE2UhTLhf5DkkIb909A6lcjHa8z1Jirp2CYS4XFchzBRUmTvbFUeARn\
                  ou5BeU6YgdUYlkGnJ9xD3loc6+HvRz09U9XxSZmy0tctC3jNMv92wSFi2kKcmt7i\
                  9xzUxj4Ia+jA8G4zr6nZ1bKSIZ4PJ2AhgU0ZC6FFtrj5+7Q+xwaWiHHqyILB35/K\
                  k7qonnRYsNjUjEV6mMWyBU1uS4JTUWYJXNDXvi08PG0tf5gKtPAvt8NixqJE43w=",
        )
        .unwrap();

        let pkcs7: Pkcs7Certificate = picky_asn1_der::from_bytes(&decoded).unwrap();
        let signer_info = pkcs7
            .signed_data
            .signers_infos
            .0
             .0
            .first()
            .expect("One SignedInfo always is present");
        let unsigned_attrs = signer_info.unsigned_attrs.0 .0.first().unwrap();
        let mc_counter_sign = match &unsigned_attrs.value {
            UnsignedAttributeValue::MsCounterSign(mc_counter_sign) => mc_counter_sign,
            _ => unreachable!(),
        };

        assert_ne!(mc_counter_sign.0.len(), 0);
        check_serde!(pkcs7: Pkcs7Certificate in decoded);
    }

    #[test]
    fn deserialize_problem_signature() {
        let decoded = base64::decode(
            "MIIjkgYJKoZIhvcNAQcCoIIjgzCCI38CAQExDzANBglghkgBZQMEAgEFADB5Bgor\
                        BgEEAYI3AgEEoGswaTA0BgorBgEEAYI3AgEeMCYCAwEAAAQQH8w7YFlLCE63JNLG\
                        KX7zUQIBAAIBAAIBAAIBAAIBADAxMA0GCWCGSAFlAwQCAQUABCBOlmcSb72K+wH5\
                        7rgEoyM/xepQH0ZFeACdfeWgW6yh06CCDYEwggX/MIID56ADAgECAhMzAAABh3IX\
                        chVZQMcJAAAAAAGHMA0GCSqGSIb3DQEBCwUAMH4xCzAJBgNVBAYTAlVTMRMwEQYD\
                        VQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNy\
                        b3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMTH01pY3Jvc29mdCBDb2RlIFNpZ25p\
                        bmcgUENBIDIwMTEwHhcNMjAwMzA0MTgzOTQ3WhcNMjEwMzAzMTgzOTQ3WjB0MQsw\
                        CQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9u\
                        ZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMR4wHAYDVQQDExVNaWNy\
                        b3NvZnQgQ29ycG9yYXRpb24wggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIB\
                        AQDOt8kLc7P3T7MKIhouYHewMFmnq8Ayu7FOhZCQabVwBp2VS4WyB2Qe4TQBT8aB\
                        znANDEPjHKNdPT8Xz5cNali6XHefS8i/WXtF0vSsP8NEv6mBHuA2p1fw2wB/F0dH\
                        sJ3GfZ5c0sPJjklsiYqPw59xJ54kM91IOgiO2OUzjNAljPibjCWfH7UzQ1TPHc4d\
                        weils8GEIrbBRb7IWwiObL12jWT4Yh71NQgvJ9Fn6+UhD9x2uk3dLj84vwt1NuFQ\
                        itKJxIV0fVsRNR3abQVOLqpDugbr0SzNL6o8xzOHL5OXiGGwg6ekiXA1/2XXY7yV\
                        Fc39tledDtZjSjNbex1zzwSXAgMBAAGjggF+MIIBejAfBgNVHSUEGDAWBgorBgEE\
                        AYI3TAgBBggrBgEFBQcDAzAdBgNVHQ4EFgQUhov4ZyO96axkJdMjpzu2zVXOJcsw\
                        UAYDVR0RBEkwR6RFMEMxKTAnBgNVBAsTIE1pY3Jvc29mdCBPcGVyYXRpb25zIFB1\
                        ZXJ0byBSaWNvMRYwFAYDVQQFEw0yMzAwMTIrNDU4Mzg1MB8GA1UdIwQYMBaAFEhu\
                        ZOVQBdOCqhc3NyK1bajKdQKVMFQGA1UdHwRNMEswSaBHoEWGQ2h0dHA6Ly93d3cu\
                        bWljcm9zb2Z0LmNvbS9wa2lvcHMvY3JsL01pY0NvZFNpZ1BDQTIwMTFfMjAxMS0w\
                        Ny0wOC5jcmwwYQYIKwYBBQUHAQEEVTBTMFEGCCsGAQUFBzAChkVodHRwOi8vd3d3\
                        Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2NlcnRzL01pY0NvZFNpZ1BDQTIwMTFfMjAx\
                        MS0wNy0wOC5jcnQwDAYDVR0TAQH/BAIwADANBgkqhkiG9w0BAQsFAAOCAgEAixmy\
                        S6E6vprWD9KFNIB9G5zyMuIjZAOuUJ1EK/Vlg6Fb3ZHXjjUwATKIcXbFuFC6Wr4K\
                        NrU4DY/sBVqmab5AC/je3bpUpjtxpEyqUqtPc30wEg/rO9vmKmqKoLPT37svc2NV\
                        BmGNl+85qO4fV/w7Cx7J0Bbqk19KcRNdjt6eKoTnTPHBHlVHQIHZpMxacbFOAkJr\
                        qAVkYZdz7ikNXTxV+GRb36tC4ByMNxE2DF7vFdvaiZP0CVZ5ByJ2gAhXMdK9+usx\
                        zVk913qKde1OAuWdv+rndqkAIm8fUlRnr4saSCg7cIbUwCCf116wUJ7EuJDg0vHe\
                        yhnCeHnBbyH3RZkHEi2ofmfgnFISJZDdMAeVZGVOh20Jp50XBzqokpPzeZ6zc1/g\
                        yILNyiVgE+RPkjnUQshd1f1PMgn3tns2Cz7bJiVUaqEO3n9qRFgy5JuLae6UweGf\
                        AeOo3dgLZxikKzYs3hDMaEtJq8IP71cX7QXe6lnMmXU/Hdfz2p897Zd+kU+vZvKI\
                        3cwLfuVQgK2RZ2z+Kc3K3dRPz2rXycK5XCuRZmvGab/WbrZiC7wJQapgBodltMI5\
                        GMdFrBg9IeF7/rP4EqVQXeKtevTlZXjpuNhhjuR+2DMt/dWufjXpiW91bo3aH6Ea\
                        jOALXmoxgltCp1K7hrS6gmsvj94cLRf50QQ4U8Qwggd6MIIFYqADAgECAgphDpDS\
                        AAAAAAADMA0GCSqGSIb3DQEBCwUAMIGIMQswCQYDVQQGEwJVUzETMBEGA1UECBMK\
                        V2FzaGluZ3RvbjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0\
                        IENvcnBvcmF0aW9uMTIwMAYDVQQDEylNaWNyb3NvZnQgUm9vdCBDZXJ0aWZpY2F0\
                        ZSBBdXRob3JpdHkgMjAxMTAeFw0xMTA3MDgyMDU5MDlaFw0yNjA3MDgyMTA5MDla\
                        MH4xCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdS\
                        ZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKDAmBgNVBAMT\
                        H01pY3Jvc29mdCBDb2RlIFNpZ25pbmcgUENBIDIwMTEwggIiMA0GCSqGSIb3DQEB\
                        AQUAA4ICDwAwggIKAoICAQCr8PpyEBwurdhuqoIQTTS68rZYIZ9CGypr6VpQqrgG\
                        OBoESbp/wwwe3TdrxhLYC/A4wpkGsMg51QEUMULTiQ15ZId+lGAkbK+eSZzpaF7S\
                        35tTsgosw6/ZqSuuegmv15ZZymAaBelmdugyUiYSL+erCFDPs0S3XdjELgN1q2jz\
                        y23zOlyhFvRGuuA4ZKxuZDV4pqBjDy3TQJP4494HDdVceaVJKecNvqATd76UPe/7\
                        4ytaEB9NViiienLgEjq3SV7Y7e1DkYPZe7J7hhvZPrGMXeiJT4Qa8qEvWeSQOy2u\
                        M1jFtz7+MtOzAz2xsq+SOH7SnYAs9U5WkSE1JcM5bmR/U7qcD60ZI4TL9LoDho33\
                        X/DQUr+MlIe8wCF0JV8YKLbMJyg4JZg5SjbPfLGSrhwjp6lm7GEfauEoSZ1fiOIl\
                        XdMhSz5SxLVXPyQD8NF6Wy/VI+NwXQ9RRnez+ADhvKwCgl/bwBWzvRvUVUvnOaEP\
                        6SNJvBi4RHxF5MHDcnrgcuck379GmcXvwhxX24ON7E1JMKerjt/sW5+v/N2wZuLB\
                        l4F77dbtS+dJKacTKKanfWeA5opieF+yL4TXV5xcv3coKPHtbcMojyyPQDdPweGF\
                        RInECUzF1KVDL3SV9274eCBYLBNdYJWaPk8zhNqwiBfenk70lrC8RqBsmNLg1oiM\
                        CwIDAQABo4IB7TCCAekwEAYJKwYBBAGCNxUBBAMCAQAwHQYDVR0OBBYEFEhuZOVQ\
                        BdOCqhc3NyK1bajKdQKVMBkGCSsGAQQBgjcUAgQMHgoAUwB1AGIAQwBBMAsGA1Ud\
                        DwQEAwIBhjAPBgNVHRMBAf8EBTADAQH/MB8GA1UdIwQYMBaAFHItOgIxkEO5FAVO\
                        4eqnxzHRI4k0MFoGA1UdHwRTMFEwT6BNoEuGSWh0dHA6Ly9jcmwubWljcm9zb2Z0\
                        LmNvbS9wa2kvY3JsL3Byb2R1Y3RzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                        Mi5jcmwwXgYIKwYBBQUHAQEEUjBQME4GCCsGAQUFBzAChkJodHRwOi8vd3d3Lm1p\
                        Y3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dDIwMTFfMjAxMV8wM18y\
                        Mi5jcnQwgZ8GA1UdIASBlzCBlDCBkQYJKwYBBAGCNy4DMIGDMD8GCCsGAQUFBwIB\
                        FjNodHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpb3BzL2RvY3MvcHJpbWFyeWNw\
                        cy5odG0wQAYIKwYBBQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AcABvAGwAaQBjAHkA\
                        XwBzAHQAYQB0AGUAbQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAGfyhqWY\
                        4FR5Gi7T2HRnIpsLlhHhY5KZQpZ90nkMkMFlXy4sPvjDctFtg/6+P+gKyju/R6mj\
                        82nbY78iNaWXXWWEkH2LRlBV2AySfNIaSxzzPEKLUtCw/WvjPgcuKZvmPRul1LUd\
                        d5Q54ulkyUQ9eHoj8xN9ppB0g430yyYCRirCihC7pKkFDJvtaPpoLpWgKj8qa1hJ\
                        Yx8JaW5amJbkg/TAj/NGK978O9C9Ne9uJa7lryft0N3zDq+ZKJeYTQ49C/IIidYf\
                        wzIY4vDFLc5bnrRJOQrGCsLGra7lstnbFYhRRVg4MnEnGn+x9Cf43iw6IGmYslmJ\
                        aG5vp7d0w0AFBqYBKig+gj8TTWYLwLNN9eGPfxxvFX1Fp3blQCplo8NdUmKGwx1j\
                        NpeG39rz+PIWoZon4c2ll9DuXWNB41sHnIc+BncG0QaxdR8UvmFhtfDcxhsEvt9B\
                        xw4o7t5lL+yX9qFcltgA1qFGvVnzl6UJS0gQmYAf0AApxbGbpT9Fdx41xtKiop96\
                        eiL6SJUfq/tHI4D1nvi/a7dLl+LrdXga7Oo3mXkYS//WsyNodeav+vyL6wuA6mk7\
                        r/ww7QRMjt/fdW1jkT3RnVZOT7+AVyKheBEyIXrvQQqxP/uozKRdwaGIm1dxVk5I\
                        RcBCyZt2WwqASGv9eZ/BvW1taslScxMNelDNMYIVZzCCFWMCAQEwgZUwfjELMAkG\
                        A1UEBhMCVVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQx\
                        HjAcBgNVBAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEoMCYGA1UEAxMfTWljcm9z\
                        b2Z0IENvZGUgU2lnbmluZyBQQ0EgMjAxMQITMwAAAYdyF3IVWUDHCQAAAAABhzAN\
                        BglghkgBZQMEAgEFAKCBrjAZBgkqhkiG9w0BCQMxDAYKKwYBBAGCNwIBBDAcBgor\
                        BgEEAYI3AgELMQ4wDAYKKwYBBAGCNwIBFTAvBgkqhkiG9w0BCQQxIgQgSI3mmyEc\
                        XjWLEpbhWFEEl6gPBJhjiWhxF4WcneiXnlYwQgYKKwYBBAGCNwIBDDE0MDKgFIAS\
                        AE0AaQBjAHIAbwBzAG8AZgB0oRqAGGh0dHA6Ly93d3cubWljcm9zb2Z0LmNvbTAN\
                        BgkqhkiG9w0BAQEFAASCAQCyr15gPEMGURRpVeQjtCEpn9waDuDlkW11PiBt2A/j\
                        PdbhN4JupkncXgZtKt29s1usM8p+bSTkao5bpeIEV5UEMxgbsaxUCipxNki+z7LW\
                        KmFzviTsUU1/CqSJ2EKZdhQENUtpmgOr0D/CHTbbAVSpiVcfQuZI8hWulziFVqRE\
                        4xGCR/sKOfQ1DT2DiOwlbf6tmceD04QaDlioZ8SVXTEvlP36a5rv8tmyw9lkkBgV\
                        B824Xh0H8CrqajF+x9zR9CjBox4Y/bf3Oe1Pir6k5IT7ZEkSQ9XRJfaNNm42i/9h\
                        IUPesYs9gr0zXJdxlri7Y2PPkphB9JQ+k+wa20nxBBIDoYIS8TCCEu0GCisGAQQB\
                        gjcDAwExghLdMIIS2QYJKoZIhvcNAQcCoIISyjCCEsYCAQMxDzANBglghkgBZQME\
                        AgEFADCCAVUGCyqGSIb3DQEJEAEEoIIBRASCAUAwggE8AgEBBgorBgEEAYRZCgMB\
                        MDEwDQYJYIZIAWUDBAIBBQAEIPeHx1THLsARquah0ml1x5Wutabkis4dsFKSE3WJ\
                        HwZlAgZfYPphXw0YEzIwMjAwOTIyMjIxOTUzLjI1NVowBIACAfSggdSkgdEwgc4x\
                        CzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRt\
                        b25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1p\
                        Y3Jvc29mdCBPcGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMg\
                        VFNTIEVTTjowQTU2LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUt\
                        U3RhbXAgU2VydmljZaCCDkQwggT1MIID3aADAgECAhMzAAABJy9uo++RqBmoAAAA\
                        AAEnMA0GCSqGSIb3DQEBCwUAMHwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNo\
                        aW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29y\
                        cG9yYXRpb24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1lLVN0YW1wIFBDQSAyMDEw\
                        MB4XDTE5MTIxOTAxMTQ1OVoXDTIxMDMxNzAxMTQ1OVowgc4xCzAJBgNVBAYTAlVT\
                        MRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQK\
                        ExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29mdCBPcGVy\
                        YXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVTTjowQTU2\
                        LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAgU2Vydmlj\
                        ZTCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAPgB3nERnk6fS40vvWeD\
                        3HCgM9Ep4xTIQiPnJXE9E+HkZVtTsPemoOyhfNAyF95E/rUvXOVTUcJFL7Xb16jT\
                        KPXONsCWY8DCixSDIiid6xa30TiEWVcIZRwiDlcx29D467OTav5rA1G6TwAEY5rQ\
                        jhUHLrOoJgfJfakZq6IHjd+slI0/qlys7QIGakFk2OB6mh/ln/nS8G4kNRK6Do4g\
                        xDtnBSFLNfhsSZlRSMDJwFvrZ2FCkaoexd7rKlUNOAAScY411IEqQeI1PwfRm3aW\
                        bS8IvAfJPC2Ah2LrtP8sKn5faaU8epexje7vZfcZif/cbxgUKStJzqbdvTBNc93n\
                        /Z8CAwEAAaOCARswggEXMB0GA1UdDgQWBBTl9JZVgF85MSRbYlOJXbhY022V8jAf\
                        BgNVHSMEGDAWgBTVYzpcijGQ80N7fEYbxTNoWoVtVTBWBgNVHR8ETzBNMEugSaBH\
                        hkVodHRwOi8vY3JsLm1pY3Jvc29mdC5jb20vcGtpL2NybC9wcm9kdWN0cy9NaWNU\
                        aW1TdGFQQ0FfMjAxMC0wNy0wMS5jcmwwWgYIKwYBBQUHAQEETjBMMEoGCCsGAQUF\
                        BzAChj5odHRwOi8vd3d3Lm1pY3Jvc29mdC5jb20vcGtpL2NlcnRzL01pY1RpbVN0\
                        YVBDQV8yMDEwLTA3LTAxLmNydDAMBgNVHRMBAf8EAjAAMBMGA1UdJQQMMAoGCCsG\
                        AQUFBwMIMA0GCSqGSIb3DQEBCwUAA4IBAQAKyo180VXHBqVnjZwQy7NlzXbo2+W5\
                        qfHxR7ANV5RBkRkdGamkwUcDNL+DpHObFPJHa0oTeYKE0Zbl1MvvfS8RtGGdhGYG\
                        CJf+BPd/gBCs4+dkZdjvOzNyuVuDPGlqQ5f7HS7iuQ/cCyGHcHYJ0nXVewF2Lk+J\
                        lrWykHpTlLwPXmCpNR+gieItPi/UMF2RYTGwojW+yIVwNyMYnjFGUxEX5/DtJjRZ\
                        mg7PBHMrENN2DgO6wBelp4ptyH2KK2EsWT+8jFCuoKv+eJby0QD55LN5f8SrUPRn\
                        K86fh7aVOfCglQofo5ABZIGiDIrg4JsV4k6p0oBSIFOAcqRAhiH+1spCMIIGcTCC\
                        BFmgAwIBAgIKYQmBKgAAAAAAAjANBgkqhkiG9w0BAQsFADCBiDELMAkGA1UEBhMC\
                        VVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNV\
                        BAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEyMDAGA1UEAxMpTWljcm9zb2Z0IFJv\
                        b3QgQ2VydGlmaWNhdGUgQXV0aG9yaXR5IDIwMTAwHhcNMTAwNzAxMjEzNjU1WhcN\
                        MjUwNzAxMjE0NjU1WjB8MQswCQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3Rv\
                        bjEQMA4GA1UEBxMHUmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0\
                        aW9uMSYwJAYDVQQDEx1NaWNyb3NvZnQgVGltZS1TdGFtcCBQQ0EgMjAxMDCCASIw\
                        DQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAKkdDbx3EYo6IOz8E5f1+n9plGt0\
                        VBDVpQoAgoX77XxoSyxfxcPlYcJ2tz5mK1vwFVMnBDEfQRsalR3OCROOfGEwWbEw\
                        RA/xYIiEVEMM1024OAizQt2TrNZzMFcmgqNFDdDq9UeBzb8kYDJYYEbyWEeGMoQe\
                        dGFnkV+BVLHPk0ySwcSmXdFhE24oxhr5hoC732H8RsEnHSRnEnIaIYqvS2SJUGKx\
                        Xf13Hz3wV3WsvYpCTUBR0Q+cBj5nf/VmwAOWRH7v0Ev9buWayrGo8noqCjHw2k4G\
                        kbaICDXoeByw6ZnNPOcvRLqn9NxkvaQBwSAJk3jN/LzAyURdXhacAQVPIk0CAwEA\
                        AaOCAeYwggHiMBAGCSsGAQQBgjcVAQQDAgEAMB0GA1UdDgQWBBTVYzpcijGQ80N7\
                        fEYbxTNoWoVtVTAZBgkrBgEEAYI3FAIEDB4KAFMAdQBiAEMAQTALBgNVHQ8EBAMC\
                        AYYwDwYDVR0TAQH/BAUwAwEB/zAfBgNVHSMEGDAWgBTV9lbLj+iiXGJo0T2UkFvX\
                        zpoYxDBWBgNVHR8ETzBNMEugSaBHhkVodHRwOi8vY3JsLm1pY3Jvc29mdC5jb20v\
                        cGtpL2NybC9wcm9kdWN0cy9NaWNSb29DZXJBdXRfMjAxMC0wNi0yMy5jcmwwWgYI\
                        KwYBBQUHAQEETjBMMEoGCCsGAQUFBzAChj5odHRwOi8vd3d3Lm1pY3Jvc29mdC5j\
                        b20vcGtpL2NlcnRzL01pY1Jvb0NlckF1dF8yMDEwLTA2LTIzLmNydDCBoAYDVR0g\
                        AQH/BIGVMIGSMIGPBgkrBgEEAYI3LgMwgYEwPQYIKwYBBQUHAgEWMWh0dHA6Ly93\
                        d3cubWljcm9zb2Z0LmNvbS9QS0kvZG9jcy9DUFMvZGVmYXVsdC5odG0wQAYIKwYB\
                        BQUHAgIwNB4yIB0ATABlAGcAYQBsAF8AUABvAGwAaQBjAHkAXwBTAHQAYQB0AGUA\
                        bQBlAG4AdAAuIB0wDQYJKoZIhvcNAQELBQADggIBAAfmiFEN4sbgmD+BcQM9naOh\
                        IW+z66bM9TG+zwXiqf76V20ZMLPCxWbJat/15/B4vceoniXj+bzta1RXCCtRgkQS\
                        +7lTjMz0YBKKdsxAQEGb3FwX/1z5Xhc1mCRWS3TvQhDIr79/xn/yN31aPxzymXlK\
                        kVIArzgPF/UveYFl2am1a+THzvbKegBvSzBEJCI8z+0DpZaPWSm8tv0E4XCfMkon\
                        /VWvL/625Y4zu2JfmttXQOnxzplmkIz/amJ/3cVKC5Em4jnsGUpxY517IW3DnKOi\
                        PPp/fZZqkHimbdLhnPkd/DjYlPTGpQqWhqS9nhquBEKDuLWAmyI4ILUl5WTs9/S/\
                        fmNZJQ96LjlXdqJxqgaKD4kWumGnEcua2A5HmoDF0M2n0O99g/DhO3EJ3110mCII\
                        YdqwUB5vvfHhAN/nMQekkzr3ZUd46PioSKv33nJ+YWtvd6mBy6cJrDm77MbL2IK0\
                        cs0d9LiFAR6A+xuJKlQ5slvayA1VmXqHczsI5pgt6o3gMy4SKfXAL1QnIffIrE7a\
                        KLixqduWsqdCosnPGUFN4Ib5KpqjEWYw07t0MkvfY3v1mYovG8chr1m1rtxEPJdQ\
                        cdeh0sVV42neV8HR3jDA/czmTfsNv11P6Z0eGTgvvM9YBS7vDaBQNdrvCScc1bN+\
                        NR4Iuto229Nfj950iEkSoYIC0jCCAjsCAQEwgfyhgdSkgdEwgc4xCzAJBgNVBAYT\
                        AlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9uMRAwDgYDVQQHEwdSZWRtb25kMR4wHAYD\
                        VQQKExVNaWNyb3NvZnQgQ29ycG9yYXRpb24xKTAnBgNVBAsTIE1pY3Jvc29mdCBP\
                        cGVyYXRpb25zIFB1ZXJ0byBSaWNvMSYwJAYDVQQLEx1UaGFsZXMgVFNTIEVTTjow\
                        QTU2LUUzMjktNEQ0RDElMCMGA1UEAxMcTWljcm9zb2Z0IFRpbWUtU3RhbXAgU2Vy\
                        dmljZaIjCgEBMAcGBSsOAwIaAxUAs5W4TmyDHMRM7iz6mgGojqvXHzOggYMwgYCk\
                        fjB8MQswCQYDVQQGEwJVUzETMBEGA1UECBMKV2FzaGluZ3RvbjEQMA4GA1UEBxMH\
                        UmVkbW9uZDEeMBwGA1UEChMVTWljcm9zb2Z0IENvcnBvcmF0aW9uMSYwJAYDVQQD\
                        Ex1NaWNyb3NvZnQgVGltZS1TdGFtcCBQQ0EgMjAxMDANBgkqhkiG9w0BAQUFAAIF\
                        AOMUsu8wIhgPMjAyMDA5MjIyMTI5MTlaGA8yMDIwMDkyMzIxMjkxOVowdzA9Bgor\
                        BgEEAYRZCgQBMS8wLTAKAgUA4xSy7wIBADAKAgEAAgIVPgIB/zAHAgEAAgIRtjAK\
                        AgUA4xYEbwIBADA2BgorBgEEAYRZCgQCMSgwJjAMBgorBgEEAYRZCgMCoAowCAIB\
                        AAIDB6EgoQowCAIBAAIDAYagMA0GCSqGSIb3DQEBBQUAA4GBAEMD4esQRMLwQdhk\
                        Co1zgvmclcwl3lYYpk1oMh1ndsU3+97Rt6FV3adS4Hezc/K94oQKjcxtMVzLzQhG\
                        agM6XlqB31VD8n2nxVuaWD1yp2jm/0IvfL9nFMHJRhgANMiBdHqvqNrd86c/Kryq\
                        sI0Ch0sOx9wg3BozzqQhmdNjf9c6MYIDDTCCAwkCAQEwgZMwfDELMAkGA1UEBhMC\
                        VVMxEzARBgNVBAgTCldhc2hpbmd0b24xEDAOBgNVBAcTB1JlZG1vbmQxHjAcBgNV\
                        BAoTFU1pY3Jvc29mdCBDb3Jwb3JhdGlvbjEmMCQGA1UEAxMdTWljcm9zb2Z0IFRp\
                        bWUtU3RhbXAgUENBIDIwMTACEzMAAAEnL26j75GoGagAAAAAAScwDQYJYIZIAWUD\
                        BAIBBQCgggFKMBoGCSqGSIb3DQEJAzENBgsqhkiG9w0BCRABBDAvBgkqhkiG9w0B\
                        CQQxIgQgcyC5Zi6T5dXlcj+V9kHGOarq/wFRtxNkp+J8JwTtAV0wgfoGCyqGSIb3\
                        DQEJEAIvMYHqMIHnMIHkMIG9BCAbkuhLEoYdahb/BUyVszO2VDi6kB3MSaof/+8u\
                        7SM+IjCBmDCBgKR+MHwxCzAJBgNVBAYTAlVTMRMwEQYDVQQIEwpXYXNoaW5ndG9u\
                        MRAwDgYDVQQHEwdSZWRtb25kMR4wHAYDVQQKExVNaWNyb3NvZnQgQ29ycG9yYXRp\
                        b24xJjAkBgNVBAMTHU1pY3Jvc29mdCBUaW1lLVN0YW1wIFBDQSAyMDEwAhMzAAAB\
                        Jy9uo++RqBmoAAAAAAEnMCIEIK4r6N3NISekswMCG1kSBJCCCePrlLDQWbMKz0wt\
                        Lj6CMA0GCSqGSIb3DQEBCwUABIIBAASNHnbCvOgFNv5zwj0UKuGscSrC0R2GxT2p\
                        H6E/QlYix36uklxd1YSqolAA30q/2BQg23N75wfA8chIgOMnaRslF9uk/oKxKHAK\
                        WezF5wx3Qoc08MJmgBQ+f/vkMUr05JIoSjgCVhlnQbO7S+aqV9ZFPDcO6IzlrmiA\
                        okZONeswosfnv1puWHRUhFJx6v3L1y+YKrRfhytDIIw1biSQ/VTO8Wnf06H0miJC\
                        1VLKNa5p8Uwx4tsWz6RvIhztN/wvOo5yUoXR55DLKUMAp283TM4A3n6exf7iEb5N\
                        4jvlHkA6au1Uan+buR92YRqCvyUjqSzSJZo7w3NwLUM6GdFUIY0=\
                        ",
        )
        .unwrap();

        let pkcs7: Pkcs7Certificate = picky_asn1_der::from_bytes(&decoded).unwrap();

        let message_digest = Attribute {
            ty: crate::oids::message_digest().into(),
            value: AttributeValues::MessageDigest(Asn1SetOf(vec![OctetStringAsn1::from(decoded[3882..3914].to_vec())])),
        };

        check_serde!(message_digest: Attribute in decoded[3865..3914]);
        check_serde!(pkcs7: Pkcs7Certificate in decoded);
    }
}
