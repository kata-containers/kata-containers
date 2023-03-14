// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::error::ErrorKind::*;
use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::str::FromStr;

/// Represents filter attributes.
///
/// You can set/get the attributes of a filter context with
/// [`ScmpFilterContext::set_filter_attr`](crate::ScmpFilterContext::set_filter_attr)
/// and [`ScmpFilterContext::get_filter_attr`](crate::ScmpFilterContext::get_filter_attr)
/// methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScmpFilterAttr {
    /// The default filter action as specified in the call to seccomp reset.
    ActDefault,
    /// The filter action taken when the loaded filter does not
    /// match the architecture of the executing application.
    ActBadArch,
    /// A flag to specify if the NO_NEW_PRIVS functionality should
    /// be enabled before loading the seccomp filter into the kernel.
    CtlNnp,
    /// A flag to specify if the kernel should attempt to
    /// synchronize the filters across all threads on seccomp load.
    CtlTsync,
    /// A flag to specify if the libseccomp should allow filter rules
    /// to be created for the -1 syscall.
    ApiTskip,
    /// A flag to specify if the kernel should log all filter
    /// actions taken except for the [`ScmpAction::Allow`](crate::ScmpAction::Allow) action.
    CtlLog,
    /// A flag to disable Speculative Store Bypass mitigations for
    /// this filter.
    CtlSsb,
    /// A flag to specify the optimization level of the seccomp
    /// filter.
    CtlOptimize,
    /// A flag to specify if the libseccomp should pass system error
    /// codes back to the caller instead of the default -ECANCELED.
    ApiSysRawRc,
}

impl ScmpFilterAttr {
    pub(crate) fn to_sys(self) -> scmp_filter_attr {
        match self {
            Self::ActDefault => scmp_filter_attr::SCMP_FLTATR_ACT_DEFAULT,
            Self::ActBadArch => scmp_filter_attr::SCMP_FLTATR_ACT_BADARCH,
            Self::CtlNnp => scmp_filter_attr::SCMP_FLTATR_CTL_NNP,
            Self::CtlTsync => scmp_filter_attr::SCMP_FLTATR_CTL_TSYNC,
            Self::ApiTskip => scmp_filter_attr::SCMP_FLTATR_API_TSKIP,
            Self::CtlLog => scmp_filter_attr::SCMP_FLTATR_CTL_LOG,
            Self::CtlSsb => scmp_filter_attr::SCMP_FLTATR_CTL_SSB,
            Self::CtlOptimize => scmp_filter_attr::SCMP_FLTATR_CTL_OPTIMIZE,
            Self::ApiSysRawRc => scmp_filter_attr::SCMP_FLTATR_API_SYSRAWRC,
        }
    }
}

impl FromStr for ScmpFilterAttr {
    type Err = SeccompError;

    /// Converts string seccomp filter attribute to `ScmpFilterAttr`.
    ///
    /// # Arguments
    ///
    /// * `attr` - A string filter attribute, e.g. `SCMP_FLTATR_*`.
    ///
    /// See the [`seccomp_attr_set(3)`] man page for details on valid filter attribute values.
    ///
    /// [`seccomp_attr_set(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_attr_set.3.html
    ///
    /// # Errors
    ///
    /// If an invalid filter attribute is specified, an error will be returned.
    fn from_str(attr: &str) -> Result<Self> {
        match attr {
            "SCMP_FLTATR_ACT_DEFAULT" => Ok(Self::ActDefault),
            "SCMP_FLTATR_ACT_BADARCH" => Ok(Self::ActBadArch),
            "SCMP_FLTATR_CTL_NNP" => Ok(Self::CtlNnp),
            "SCMP_FLTATR_CTL_TSYNC" => Ok(Self::CtlTsync),
            "SCMP_FLTATR_API_TSKIP" => Ok(Self::ApiTskip),
            "SCMP_FLTATR_CTL_LOG" => Ok(Self::CtlLog),
            "SCMP_FLTATR_CTL_SSB" => Ok(Self::CtlSsb),
            "SCMP_FLTATR_CTL_OPTIMIZE" => Ok(Self::CtlOptimize),
            "SCMP_FLTATR_API_SYSRAWRC" => Ok(Self::ApiSysRawRc),
            _ => Err(SeccompError::new(ParseError)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filter_attr() {
        let test_data = [
            ("SCMP_FLTATR_ACT_DEFAULT", ScmpFilterAttr::ActDefault),
            ("SCMP_FLTATR_ACT_BADARCH", ScmpFilterAttr::ActBadArch),
            ("SCMP_FLTATR_CTL_NNP", ScmpFilterAttr::CtlNnp),
            ("SCMP_FLTATR_CTL_TSYNC", ScmpFilterAttr::CtlTsync),
            ("SCMP_FLTATR_API_TSKIP", ScmpFilterAttr::ApiTskip),
            ("SCMP_FLTATR_CTL_LOG", ScmpFilterAttr::CtlLog),
            ("SCMP_FLTATR_CTL_SSB", ScmpFilterAttr::CtlSsb),
            ("SCMP_FLTATR_CTL_OPTIMIZE", ScmpFilterAttr::CtlOptimize),
            ("SCMP_FLTATR_API_SYSRAWRC", ScmpFilterAttr::ApiSysRawRc),
        ];
        for data in test_data {
            assert_eq!(
                ScmpFilterAttr::from_str(data.0).unwrap().to_sys(),
                data.1.to_sys()
            );
        }
        assert!(ScmpFilterAttr::from_str("SCMP_INVALID_FLAG").is_err());
    }
}
