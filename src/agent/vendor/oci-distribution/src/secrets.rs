//! Types for working with registry access secrets

/// A method for authenticating to a registry
#[derive(Eq, PartialEq, Debug, Clone)]
pub enum RegistryAuth {
    /// Access the registry anonymously
    Anonymous,
    /// Access the registry using HTTP Basic authentication
    Basic(String, String),
}

pub(crate) trait Authenticable {
    fn apply_authentication(self, auth: &RegistryAuth) -> Self;
}

impl Authenticable for reqwest::RequestBuilder {
    fn apply_authentication(self, auth: &RegistryAuth) -> Self {
        match auth {
            RegistryAuth::Anonymous => self,
            RegistryAuth::Basic(username, password) => self.basic_auth(username, Some(password)),
        }
    }
}
