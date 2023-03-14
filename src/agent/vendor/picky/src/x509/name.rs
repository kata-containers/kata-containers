use oid::ObjectIdentifier;
use picky_asn1::restricted_string::{CharSetError, IA5String};
use picky_asn1::wrapper::{Asn1SequenceOf, IA5StringAsn1};
use picky_asn1_x509::{
    DirectoryString, GeneralName as SerdeGeneralName, GeneralNames as SerdeGeneralNames, Name, NamePrettyFormatter,
    OtherName,
};
use std::fmt;

// === DirectoryName ===

pub use picky_asn1_x509::NameAttr;

#[derive(Clone, Debug, PartialEq)]
pub struct DirectoryName(Name);

impl Default for DirectoryName {
    fn default() -> Self {
        Self::new()
    }
}

impl DirectoryName {
    pub fn new() -> Self {
        Self(Name::new())
    }

    pub fn new_common_name<S: Into<DirectoryString>>(name: S) -> Self {
        Self(Name::new_common_name(name))
    }

    /// Find the first common name contained in this `Name`
    pub fn find_common_name(&self) -> Option<&DirectoryString> {
        self.0.find_common_name()
    }

    pub fn add_attr<S: Into<DirectoryString>>(&mut self, attr: NameAttr, value: S) {
        self.0.add_attr(attr, value)
    }

    /// Add an emailAddress attribute.
    /// NOTE: this attribute does not conform with the RFC 5280, email should be placed in SAN instead
    pub fn add_email<S: Into<IA5StringAsn1>>(&mut self, value: S) {
        self.0.add_email(value)
    }
}

impl fmt::Display for DirectoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        NamePrettyFormatter(&self.0).fmt(f)
    }
}

impl From<Name> for DirectoryName {
    fn from(name: Name) -> Self {
        Self(name)
    }
}

impl From<DirectoryName> for Name {
    fn from(name: DirectoryName) -> Self {
        name.0
    }
}

// === GeneralNames === //

#[derive(Debug, PartialEq, Clone)]
pub enum GeneralName {
    OtherName(OtherName),
    RFC822Name(IA5String),
    DNSName(IA5String),
    DirectoryName(DirectoryName),
    EDIPartyName {
        name_assigner: Option<DirectoryString>,
        party_name: DirectoryString,
    },
    URI(IA5String),
    IpAddress(Vec<u8>),
    RegisteredId(ObjectIdentifier),
}

impl GeneralName {
    pub fn new_rfc822_name<S: Into<String>>(name: S) -> Result<Self, CharSetError> {
        Ok(Self::RFC822Name(IA5String::from_string(name.into())?))
    }

    pub fn new_dns_name<S: Into<String>>(name: S) -> Result<Self, CharSetError> {
        Ok(Self::DNSName(IA5String::from_string(name.into())?))
    }

    pub fn new_directory_name<N: Into<DirectoryName>>(name: N) -> Self {
        Self::DirectoryName(name.into())
    }

    pub fn new_edi_party_name<PN, NA>(party_name: PN, name_assigner: Option<NA>) -> Self
    where
        PN: Into<DirectoryString>,
        NA: Into<DirectoryString>,
    {
        Self::EDIPartyName {
            name_assigner: name_assigner.map(Into::into),
            party_name: party_name.into(),
        }
    }

    pub fn new_uri<S: Into<String>>(uri: S) -> Result<Self, CharSetError> {
        Ok(Self::URI(IA5String::from_string(uri.into())?))
    }

    pub fn new_ip_address<ADDR: Into<Vec<u8>>>(ip_address: ADDR) -> Self {
        Self::IpAddress(ip_address.into())
    }

    pub fn new_registered_id<OID: Into<ObjectIdentifier>>(oid: OID) -> Self {
        Self::RegisteredId(oid.into())
    }
}

impl From<SerdeGeneralName> for GeneralName {
    fn from(gn: SerdeGeneralName) -> Self {
        match gn {
            SerdeGeneralName::OtherName(other_name) => Self::OtherName(other_name),
            SerdeGeneralName::Rfc822Name(name) => Self::RFC822Name(name.0),
            SerdeGeneralName::DnsName(name) => Self::DNSName(name.0),
            SerdeGeneralName::DirectoryName(name) => Self::DirectoryName(name.into()),
            SerdeGeneralName::EdiPartyName(edi_pn) => Self::EDIPartyName {
                name_assigner: edi_pn.name_assigner.0.map(|na| na.0),
                party_name: edi_pn.party_name.0,
            },
            SerdeGeneralName::Uri(uri) => Self::URI(uri.0),
            SerdeGeneralName::IpAddress(ip_addr) => Self::IpAddress(ip_addr.0),
            SerdeGeneralName::RegisteredId(id) => Self::RegisteredId(id.0),
        }
    }
}

impl From<GeneralName> for SerdeGeneralName {
    fn from(gn: GeneralName) -> Self {
        match gn {
            GeneralName::OtherName(other_name) => SerdeGeneralName::OtherName(other_name),
            GeneralName::RFC822Name(name) => SerdeGeneralName::Rfc822Name(name.into()),
            GeneralName::DNSName(name) => SerdeGeneralName::DnsName(name.into()),
            GeneralName::DirectoryName(name) => SerdeGeneralName::DirectoryName(name.into()),
            GeneralName::EDIPartyName {
                name_assigner,
                party_name,
            } => SerdeGeneralName::new_edi_party_name(party_name, name_assigner),
            GeneralName::URI(uri) => SerdeGeneralName::Uri(uri.into()),
            GeneralName::IpAddress(ip_addr) => SerdeGeneralName::IpAddress(ip_addr.into()),
            GeneralName::RegisteredId(id) => SerdeGeneralName::RegisteredId(id.into()),
        }
    }
}

impl From<GeneralName> for SerdeGeneralNames {
    fn from(gn: GeneralName) -> Self {
        GeneralNames::new(gn).into()
    }
}

/// Wraps x509 `GeneralNames` into an easy to use API.
///
/// # Example
///
/// ```
/// use picky::x509::name::{GeneralNames, GeneralName, DirectoryName};
///
/// let common_name = GeneralName::new_directory_name(DirectoryName::new_common_name("MyName"));
/// let dns_name = GeneralName::new_dns_name("localhost").expect("invalid name string");
/// let names = GeneralNames::from(vec![common_name, dns_name]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralNames(SerdeGeneralNames);

impl GeneralNames {
    /// # Example
    ///
    /// ```
    /// use picky::x509::name::{GeneralName, GeneralNames};
    ///
    /// let dns_name = GeneralName::new_dns_name("localhost").expect("invalid name string");
    /// let names = GeneralNames::new(dns_name);
    /// ```
    pub fn new<GN: Into<GeneralName>>(gn: GN) -> Self {
        let gn = gn.into();
        Self(Asn1SequenceOf(vec![gn.into()]))
    }

    pub fn new_directory_name<DN: Into<DirectoryName>>(name: DN) -> Self {
        let gn = GeneralName::new_directory_name(name);
        Self::new(gn)
    }

    pub fn with_directory_name<DN: Into<DirectoryName>>(mut self, name: DN) -> Self {
        let gn = GeneralName::new_directory_name(name);
        (self.0).0.push(gn.into());
        self
    }

    pub fn find_directory_name(&self) -> Option<DirectoryName> {
        for name in &(self.0).0 {
            if let SerdeGeneralName::DirectoryName(name) = name {
                return Some(name.clone().into());
            }
        }
        None
    }

    /// # Example
    ///
    /// ```
    /// use picky::x509::name::GeneralNames;
    /// use picky_asn1::restricted_string::IA5String;
    ///
    /// let names = GeneralNames::new_dns_name(IA5String::new("localhost").unwrap());
    /// ```
    pub fn new_dns_name<IA5: Into<IA5String>>(dns_name: IA5) -> Self {
        let gn = GeneralName::DNSName(dns_name.into());
        Self::new(gn)
    }

    /// # Example
    ///
    /// ```
    /// use picky::x509::name::{GeneralNames, DirectoryName};
    /// use picky_asn1::restricted_string::IA5String;
    ///
    /// let names = GeneralNames::new_directory_name(DirectoryName::new_common_name("MyName"))
    ///         .with_dns_name(IA5String::new("localhost").unwrap());
    /// ```
    pub fn with_dns_name<IA5: Into<IA5String>>(mut self, dns_name: IA5) -> Self {
        let gn = GeneralName::DNSName(dns_name.into());
        (self.0).0.push(gn.into());
        self
    }

    pub fn find_dns_name(&self) -> Option<&IA5String> {
        for name in &(self.0).0 {
            if let SerdeGeneralName::DnsName(name) = name {
                return Some(&name.0);
            }
        }
        None
    }

    pub fn add_name<GN: Into<GeneralName>>(&mut self, name: GN) {
        let gn = name.into();
        (self.0).0.push(gn.into());
    }

    /// # Example
    ///
    /// ```
    /// use picky::x509::name::{GeneralNames, GeneralName, DirectoryName};
    ///
    /// let common_name = GeneralName::new_directory_name(DirectoryName::new_common_name("MyName"));
    /// let dns_name = GeneralName::new_dns_name("localhost").expect("invalid name string");
    /// let names = GeneralNames::new(common_name).with_name(dns_name);
    /// ```
    pub fn with_name<GN: Into<GeneralName>>(mut self, name: GN) -> Self {
        let gn = name.into();
        (self.0).0.push(gn.into());
        self
    }

    pub fn into_general_names(self) -> Vec<GeneralName> {
        (self.0).0.into_iter().map(|gn| gn.into()).collect()
    }

    pub fn to_general_names(&self) -> Vec<GeneralName> {
        (self.0).0.iter().map(|gn| gn.clone().into()).collect()
    }
}

impl From<SerdeGeneralNames> for GeneralNames {
    fn from(gn: SerdeGeneralNames) -> Self {
        Self(gn)
    }
}

impl From<GeneralNames> for SerdeGeneralNames {
    fn from(gn: GeneralNames) -> Self {
        gn.0
    }
}

impl From<Vec<GeneralName>> for GeneralNames {
    fn from(names: Vec<GeneralName>) -> Self {
        let serde_names = names.into_iter().map(|n| n.into()).collect();
        Self(Asn1SequenceOf(serde_names))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_format_directory_name() {
        let mut my_name = DirectoryName::new_common_name("CommonName");
        my_name.add_attr(NameAttr::StateOrProvinceName, "SomeState");
        my_name.add_attr(NameAttr::CountryName, "SomeCountry");
        assert_eq!(my_name.to_string(), "CN=CommonName,ST=SomeState,C=SomeCountry");
    }

    #[test]
    fn find_common_name() {
        let my_name = DirectoryName::new_common_name("CommonName");
        let cn = my_name.find_common_name().unwrap();
        assert_eq!(cn.to_utf8_lossy(), "CommonName");
    }
}
