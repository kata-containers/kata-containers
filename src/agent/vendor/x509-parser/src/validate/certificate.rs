use crate::certificate::*;
use crate::validate::*;

use extensions::X509ExtensionsValidator;

#[derive(Debug)]
pub struct X509CertificateValidator;

impl<'a> Validator<'a> for X509CertificateValidator {
    type Item = X509Certificate<'a>;

    fn validate<L: Logger>(&self, item: &'a Self::Item, l: &'_ mut L) -> bool {
        let mut res = true;
        res &= X509ExtensionsValidator.validate(&item.extensions(), l);
        res
    }
}
