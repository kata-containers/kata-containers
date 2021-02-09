use std::error::Error;
use std::fmt;
use Fail;

pub struct BoxStd(pub Box<dyn Error + Send + Sync + 'static>);

impl fmt::Display for BoxStd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl fmt::Debug for BoxStd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Fail for BoxStd {}
