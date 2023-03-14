// SPDX-License-Identifier: Apache-2.0 or MIT
//
// Copyright 2021 Sony Group Corporation
//

use crate::api::ensure_supported_api;
use crate::error::{Result, SeccompError};
use libseccomp_sys::*;
use std::os::unix::io::AsRawFd;
use std::ptr::NonNull;

use crate::*;

const MINUS_EEXIST: i32 = -libc::EEXIST;

/// **Represents a filter context in the libseccomp.**
#[derive(Debug)]
pub struct ScmpFilterContext {
    ctx: NonNull<libc::c_void>,
}

impl ScmpFilterContext {
    /// Creates and returns a new filter context.
    ///
    /// This initializes the internal seccomp filter state and should
    /// be called before any other functions in this crate to ensure the filter
    /// state is initialized.
    ///
    /// This function returns a valid filter context.
    ///
    /// This function corresponds to
    /// [`seccomp_init`](https://man7.org/linux/man-pages/man3/seccomp_init.3.html).
    ///
    /// # Arguments
    ///
    /// * `default_action` - A default action to be taken for syscalls which match no rules in the filter
    ///
    /// # Errors
    ///
    /// If the filter context can not be created, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new_filter(default_action: ScmpAction) -> Result<Self> {
        let ctx_ptr = unsafe { seccomp_init(default_action.to_sys()) };
        let ctx = NonNull::new(ctx_ptr)
            .ok_or_else(|| SeccompError::with_msg("Could not create new filter"))?;

        Ok(Self { ctx })
    }

    /// Merges two filters.
    ///
    /// In order to merge two seccomp filters, both filters must have the same
    /// attribute values and no overlapping architectures.
    /// If successful, the `src` seccomp filter is released and all internal memory
    /// associated with the filter is freed.
    ///
    /// This function corresponds to
    /// [`seccomp_merge`](https://man7.org/linux/man-pages/man3/seccomp_merge.3.html).
    ///
    /// # Arguments
    ///
    /// * `src` - A seccomp filter that will be merged into the filter this is called on.
    ///
    /// # Errors
    ///
    /// If merging the filters fails, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx1 = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let mut ctx2 = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// if !ctx1.is_arch_present(ScmpArch::X8664)? {
    ///     ctx1.add_arch(ScmpArch::X8664)?;
    ///     ctx1.remove_arch(ScmpArch::Native)?;
    /// }
    /// if !ctx2.is_arch_present(ScmpArch::Aarch64)? {
    ///     ctx2.add_arch(ScmpArch::Aarch64)?;
    ///     ctx2.remove_arch(ScmpArch::Native)?;
    /// }
    /// ctx1.merge(ctx2)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn merge(&mut self, src: Self) -> Result<()> {
        cvt(unsafe { seccomp_merge(self.ctx.as_ptr(), src.ctx.as_ptr()) })?;

        // The src filter is already released.
        std::mem::forget(src);

        Ok(())
    }

    /// Checks if an architecture is present in a filter.
    ///
    /// If a filter contains an architecture, it uses its default action for
    /// syscalls which do not match rules in it, and its rules can match syscalls
    /// for that ABI.
    /// If a filter does not contain an architecture, all syscalls made to that
    /// kernel ABI will fail with the filter's default Bad Architecture Action
    /// (by default, killing the proc).
    ///
    /// This function returns `Ok(true)` if the architecture is present in the filter,
    /// `Ok(false)` otherwise.
    ///
    /// This function corresponds to
    /// [`seccomp_arch_exist`](https://man7.org/linux/man-pages/man3/seccomp_arch_exist.3.html).
    ///
    /// # Arguments
    ///
    /// * `arch` - An architecture token
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered calling to the libseccomp API, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.add_arch(ScmpArch::Aarch64)?;
    /// assert!(ctx.is_arch_present(ScmpArch::Aarch64)?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn is_arch_present(&self, arch: ScmpArch) -> Result<bool> {
        match unsafe { seccomp_arch_exist(self.ctx.as_ptr(), arch.to_sys()) } {
            0 => Ok(true),
            MINUS_EEXIST => Ok(false),
            errno => Err(SeccompError::from_errno(errno)),
        }
    }

    /// Adds an architecture to the filter.
    ///
    /// This function returns `Ok(true)` if the architecture was added to the
    /// filter and `Ok(false)` if the architecture was already present in the
    /// filter.
    ///
    /// This function corresponds to
    /// [`seccomp_arch_add`](https://man7.org/linux/man-pages/man3/seccomp_arch_add.3.html).
    ///
    /// # Arguments
    ///
    /// * `arch` - An architecture token
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered adding the architecture, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.add_arch(ScmpArch::X86)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_arch(&mut self, arch: ScmpArch) -> Result<bool> {
        match unsafe { seccomp_arch_add(self.ctx.as_ptr(), arch.to_sys()) } {
            0 => Ok(true),
            MINUS_EEXIST => Ok(false),
            errno => Err(SeccompError::from_errno(errno)),
        }
    }

    /// Removes an architecture from the filter.
    ///
    /// This function returns `Ok(true)` if the architecture was removed from
    /// the filter and `Ok(false)` if the architecture wasn't present in the
    /// filter.
    ///
    /// This function corresponds to
    /// [`seccomp_arch_remove`](https://man7.org/linux/man-pages/man3/seccomp_arch_remove.3.html).
    ///
    /// # Arguments
    ///
    /// * `arch` - An architecture token
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered removing the architecture, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.add_arch(ScmpArch::X86)?;
    /// ctx.remove_arch(ScmpArch::X86)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn remove_arch(&mut self, arch: ScmpArch) -> Result<bool> {
        match unsafe { seccomp_arch_remove(self.ctx.as_ptr(), arch.to_sys()) } {
            0 => Ok(true),
            MINUS_EEXIST => Ok(false),
            errno => Err(SeccompError::from_errno(errno)),
        }
    }

    /// Adds a single rule for an unconditional action on a syscall.
    ///
    /// If the specified rule needs to be rewritten due to architecture specifics,
    /// it will be rewritten without notification.
    ///
    /// This function corresponds to
    /// [`seccomp_rule_add`](https://man7.org/linux/man-pages/man3/seccomp_rule_add.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - An action to be taken on the call being made
    /// * `syscall` - The number of syscall
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered adding the rule, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("ptrace")?;
    /// ctx.add_rule(ScmpAction::Errno(libc::EPERM), syscall)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_rule<S: Into<ScmpSyscall>>(&mut self, action: ScmpAction, syscall: S) -> Result<()> {
        self.add_rule_conditional(action, syscall, &[])
    }

    /// Adds a single rule for a conditional action on a syscall.
    ///
    /// If the specified rule needs to be rewritten due to architecture specifics,
    /// it will be rewritten without notification.
    /// Comparators are AND'd together (i.e. all must match for the rule to match).
    /// You can only compare each argument once in a single rule.
    ///
    /// This function corresponds to
    /// [`seccomp_rule_add_array`](https://man7.org/linux/man-pages/man3/seccomp_rule_add_array.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - An action to be taken on the call being made
    /// * `syscall` - The number of syscall
    /// * `comparators` - An array of the rule in a seccomp filter
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered adding the rule, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("open")?;
    /// ctx.add_rule_conditional(
    ///     ScmpAction::Errno(libc::EPERM),
    ///     syscall,
    ///     &[scmp_cmp!($arg1 & (libc::O_TRUNC as u64) == libc::O_TRUNC as u64)],
    /// )?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_rule_conditional<S: Into<ScmpSyscall>>(
        &mut self,
        action: ScmpAction,
        syscall: S,
        comparators: &[ScmpArgCompare],
    ) -> Result<()> {
        cvt(unsafe {
            seccomp_rule_add_array(
                self.ctx.as_ptr(),
                action.to_sys(),
                syscall.into().to_sys(),
                comparators.len() as u32,
                comparators.as_ptr().cast::<scmp_arg_cmp>(),
            )
        })
    }

    /// Adds a single rule for an unconditional action on a syscall.
    ///
    /// The functions will attempt to add the rule exactly as specified so it may
    /// behave differently on different architectures.
    /// If the specified rule can not be represented on the architecture,
    /// the function will fail.
    ///
    /// This function corresponds to
    /// [`seccomp_rule_add_exact`](https://man7.org/linux/man-pages/man3/seccomp_rule_add_exact.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - An action to be taken on the call being made
    /// * `syscall` - The number of syscall
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered adding the rule, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("dup3")?;
    /// ctx.add_rule_exact(ScmpAction::KillThread, syscall)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_rule_exact<S: Into<ScmpSyscall>>(
        &mut self,
        action: ScmpAction,
        syscall: S,
    ) -> Result<()> {
        self.add_rule_conditional_exact(action, syscall, &[])
    }

    /// Adds a single rule for a conditional action on a syscall.
    ///
    /// The functions will attempt to add the rule exactly as specified so it may
    /// behave differently on different architectures.
    /// If the specified rule can not be represented on the architecture,
    /// the function will fail.
    ///
    /// This function corresponds to
    /// [`seccomp_rule_add_exact_array`](https://man7.org/linux/man-pages/man3/seccomp_rule_add_exact_array.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - An action to be taken on the call being made
    /// * `syscall` - The number of syscall
    /// * `comparators` - An array of the rule in a seccomp filter
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered adding the rule, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("socket")?;
    /// ctx.add_rule_conditional_exact(
    ///     ScmpAction::Errno(libc::EPERM),
    ///     syscall,
    ///     &[scmp_cmp!($arg1 != libc::AF_UNIX as u64)],
    /// )?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn add_rule_conditional_exact<S: Into<ScmpSyscall>>(
        &mut self,
        action: ScmpAction,
        syscall: S,
        comparators: &[ScmpArgCompare],
    ) -> Result<()> {
        cvt(unsafe {
            seccomp_rule_add_exact_array(
                self.ctx.as_ptr(),
                action.to_sys(),
                syscall.into().to_sys(),
                comparators.len() as u32,
                comparators.as_ptr().cast::<scmp_arg_cmp>(),
            )
        })
    }

    /// Loads a filter context into the kernel.
    ///
    /// If the function succeeds, the new filter will be active when the function returns.
    ///
    /// This function corresponds to
    /// [`seccomp_load`](https://man7.org/linux/man-pages/man3/seccomp_load.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered loading the rule, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("dup3")?;
    /// ctx.add_rule(ScmpAction::KillThread, syscall)?;
    /// ctx.load()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load(&self) -> Result<()> {
        cvt(unsafe { seccomp_load(self.ctx.as_ptr()) })
    }

    /// Sets a syscall's priority.
    ///
    /// This provides a priority hint to the seccomp filter generator in the libseccomp
    /// such that higher priority syscalls are placed earlier in the seccomp filter code
    /// so that they incur less overhead at the expense of lower priority syscalls.
    ///
    /// This function corresponds to
    /// [`seccomp_syscall_priority`](https://man7.org/linux/man-pages/man3/seccomp_syscall_priority.3.html).
    ///
    /// # Arguments
    ///
    /// * `syscall` - The number of syscall
    /// * `priority` - The priority parameter that is an 8-bit value ranging from 0 to 255;
    /// a higher value represents a higher priority.
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or the number of syscall
    /// is invalid, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let syscall = ScmpSyscall::from_name("open")?;
    /// ctx.set_syscall_priority(syscall, 100)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_syscall_priority<S: Into<ScmpSyscall>>(
        &mut self,
        syscall: S,
        priority: u8,
    ) -> Result<()> {
        cvt(unsafe {
            seccomp_syscall_priority(self.ctx.as_ptr(), syscall.into().to_sys(), priority)
        })
    }

    /// Gets a raw filter attribute value.
    ///
    /// The seccomp filter attributes are tunable values that affect how the library behaves
    /// when generating and loading the seccomp filter into the kernel.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Arguments
    ///
    /// * `attr` - A seccomp filter attribute
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered retrieving the attribute, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// assert_ne!(ctx.get_filter_attr(ScmpFilterAttr::CtlNnp)?, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_filter_attr(&self, attr: ScmpFilterAttr) -> Result<u32> {
        let mut attribute: u32 = 0;

        cvt(unsafe { seccomp_attr_get(self.ctx.as_ptr(), attr.to_sys(), &mut attribute) })?;

        Ok(attribute)
    }

    /// Gets the default action as specified in the call to
    /// [`new_filter()`](ScmpFilterContext::new_filter) or [`reset()`](ScmpFilterContext::reset).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered getting the action, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let action = ctx.get_act_default()?;
    /// assert_eq!(action, ScmpAction::Allow);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_act_default(&self) -> Result<ScmpAction> {
        let ret = self.get_filter_attr(ScmpFilterAttr::ActDefault)?;

        ScmpAction::from_sys(ret)
    }

    /// Gets the default action taken when the loaded filter does not match the architecture
    /// of the executing application.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered getting the action, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// let action = ctx.get_act_badarch()?;
    /// assert_eq!(action, ScmpAction::KillThread);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_act_badarch(&self) -> Result<ScmpAction> {
        let ret = self.get_filter_attr(ScmpFilterAttr::ActBadArch)?;

        ScmpAction::from_sys(ret)
    }

    /// Gets the current state of the [`ScmpFilterAttr::CtlNnp`] attribute.
    ///
    /// This function returns `Ok(true)` if the [`ScmpFilterAttr::CtlNnp`] attribute is set to on the filter being
    /// loaded, `Ok(false)` otherwise.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered getting the current state, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_nnp(false)?;
    /// assert!(!ctx.get_ctl_nnp()?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_ctl_nnp(&self) -> Result<bool> {
        let ret = self.get_filter_attr(ScmpFilterAttr::CtlNnp)?;

        Ok(ret != 0)
    }

    /// Deprecated alias for [`ScmpFilterContext::get_ctl_nnp()`].
    #[deprecated(since = "0.2.3", note = "Use ScmpFilterContext::get_ctl_nnp().")]
    pub fn get_no_new_privs_bit(&self) -> Result<bool> {
        self.get_ctl_nnp()
    }

    /// Gets the current state of the [`ScmpFilterAttr::CtlTsync`] attribute.
    ///
    /// This function returns `Ok(true)` if the [`ScmpFilterAttr::CtlTsync`] attribute set to on the filter being
    /// loaded, `Ok(false)` otherwise.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// getting the current state, or the libseccomp API level is less than 2, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_tsync(true)?;
    /// assert!(ctx.get_ctl_tsync()?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_ctl_tsync(&self) -> Result<bool> {
        ensure_supported_api("get_ctl_tsync", 2, ScmpVersion::from((2, 2, 0)))?;
        let ret = self.get_filter_attr(ScmpFilterAttr::CtlTsync)?;

        Ok(ret != 0)
    }

    /// Gets the current state of the [`ScmpFilterAttr::CtlLog`] attribute.
    ///
    /// This function returns `Ok(true)` if the [`ScmpFilterAttr::CtlLog`] attribute set to on the filter being
    /// loaded, `Ok(false)` otherwise.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// getting the current state, or the libseccomp API level is less than 3, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_log(true)?;
    /// assert!(ctx.get_ctl_log()?);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_ctl_log(&self) -> Result<bool> {
        ensure_supported_api("get_ctl_log", 3, ScmpVersion::from((2, 4, 0)))?;
        let ret = self.get_filter_attr(ScmpFilterAttr::CtlLog)?;

        Ok(ret != 0)
    }

    /// Gets the current state of the [`ScmpFilterAttr::CtlSsb`] attribute.
    ///
    /// This function returns `Ok(true)` if the [`ScmpFilterAttr::CtlSsb`] attribute set to on the filter being
    /// loaded, `Ok(false)` otherwise.
    /// The [`ScmpFilterAttr::CtlSsb`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// getting the current state, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0))).unwrap() {
    /// ctx.set_ctl_ssb(false)?;
    /// assert!(!ctx.get_ctl_ssb()?);
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_ctl_ssb(&self) -> Result<bool> {
        ensure_supported_api("get_ctl_ssb", 4, ScmpVersion::from((2, 5, 0)))?;
        let ret = self.get_filter_attr(ScmpFilterAttr::CtlSsb)?;

        Ok(ret != 0)
    }

    /// Gets the current optimization level of the [`ScmpFilterAttr::CtlOptimize`] attribute.
    ///
    /// See [`set_ctl_optimize()`](ScmpFilterContext::set_ctl_optimize) for more details about
    /// the optimization level.
    /// The [`ScmpFilterAttr::CtlOptimize`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// getting the current state, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0)))? {
    /// ctx.set_ctl_optimize(2)?;
    /// assert_eq!(ctx.get_ctl_optimize()?, 2);
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_ctl_optimize(&self) -> Result<u32> {
        ensure_supported_api("get_ctl_optimize", 4, ScmpVersion::from((2, 5, 0)))?;
        let ret = self.get_filter_attr(ScmpFilterAttr::CtlOptimize)?;

        Ok(ret)
    }

    /// Gets the current state of the [`ScmpFilterAttr::ApiSysRawRc`] attribute.
    ///
    /// This function returns `Ok(true)` if the [`ScmpFilterAttr::ApiSysRawRc`] attribute set to on the filter
    /// being loaded, `Ok(false)` otherwise.
    /// The [`ScmpFilterAttr::ApiSysRawRc`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_get`](https://man7.org/linux/man-pages/man3/seccomp_attr_get.3.html).
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// getting the current state, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0)))? {
    /// ctx.set_api_sysrawrc(true)?;
    /// assert!(ctx.get_api_sysrawrc()?);
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_api_sysrawrc(&self) -> Result<bool> {
        ensure_supported_api("get_api_sysrawrc", 4, ScmpVersion::from((2, 5, 0)))?;
        let ret = self.get_filter_attr(ScmpFilterAttr::ApiSysRawRc)?;

        Ok(ret != 0)
    }

    /// Sets a raw filter attribute value.
    ///
    /// The seccomp filter attributes are tunable values that affect how the library behaves
    /// when generating and loading the seccomp filter into the kernel.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `attr` - A seccomp filter attribute
    /// * `value` - A value of or the parameter of the attribute
    ///
    /// See the [`seccomp_attr_set(3)`] man page for details on available attribute values.
    ///
    /// [`seccomp_attr_set(3)`]: https://www.man7.org/linux/man-pages/man3/seccomp_attr_set.3.html
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered setting the attribute, an error will be returned.
    pub fn set_filter_attr(&mut self, attr: ScmpFilterAttr, value: u32) -> Result<()> {
        cvt(unsafe { seccomp_attr_set(self.ctx.as_ptr(), attr.to_sys(), value) })
    }

    /// Sets the default action taken when the loaded filter does not match the architecture
    /// of the executing application.
    ///
    /// Defaults to on (`action` == [`ScmpAction::KillThread`]).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - An action to be taken on a syscall for an architecture not in the filter.
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered setting the attribute, an error will be returned.
    ///
    /// # Examples
    ///
    ///  ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_act_badarch(ScmpAction::KillProcess)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_act_badarch(&mut self, action: ScmpAction) -> Result<()> {
        self.set_filter_attr(ScmpFilterAttr::ActBadArch, action.to_sys())
    }

    /// Sets the state of the [`ScmpFilterAttr::CtlNnp`] attribute which will be applied
    /// on filter load.
    ///
    /// Settings this to off (`state` == `false`) means that loading the seccomp filter
    /// into the kernel fill fail if the `CAP_SYS_ADMIN` is missing.
    ///
    /// Defaults to on (`state` == `true`).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `state` - A state flag to specify whether the [`ScmpFilterAttr::CtlNnp`] attribute should be enabled
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is
    /// encountered setting the attribute, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_nnp(false)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_ctl_nnp(&mut self, state: bool) -> Result<()> {
        self.set_filter_attr(ScmpFilterAttr::CtlNnp, state.into())
    }

    /// Deprecated alias for [`ScmpFilterContext::set_ctl_nnp()`].
    #[deprecated(since = "0.2.3", note = "Use ScmpFilterContext::set_ctl_nnp().")]
    pub fn set_no_new_privs_bit(&mut self, state: bool) -> Result<()> {
        self.set_ctl_nnp(state)
    }

    /// Sets the state of the [`ScmpFilterAttr::CtlTsync`] attribute which will be applied
    /// on filter load.
    ///
    /// Settings this to on (`state` == `true`) means that the kernel should attempt to synchronize the filters
    /// across all threads on [`ScmpFilterContext::load()`].
    /// If the kernel is unable to synchronize all of the thread then the load operation will fail.
    /// The [`ScmpFilterAttr::CtlTsync`] attribute is only usable when the libseccomp API level 2 or higher
    /// is supported.
    /// If the libseccomp API level is less than 6, the [`ScmpFilterAttr::CtlTsync`] attribute is unusable
    /// with the userspace notification API simultaneously.
    ///
    /// Defaults to off (`state` == `false`).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `state` - A state flag to specify whether the [`ScmpFilterAttr::CtlTsync`] attribute should be enabled
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// setting the attribute, or the libseccomp API level is less than 2, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_tsync(true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_ctl_tsync(&mut self, state: bool) -> Result<()> {
        ensure_supported_api("set_ctl_tsync", 2, ScmpVersion::from((2, 2, 0)))?;
        self.set_filter_attr(ScmpFilterAttr::CtlTsync, state.into())
    }

    /// Sets the state of the [`ScmpFilterAttr::CtlLog`] attribute which will be applied on filter load.
    ///
    /// Settings this to on (`state` == `true`) means that the kernel should log all filter
    /// actions taken except for the [`ScmpAction::Allow`] action.
    /// The [`ScmpFilterAttr::CtlLog`] attribute is only usable when the libseccomp API level 3 or higher
    /// is supported.
    ///
    /// Defaults to off (`state` == `false`).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `state` - A state flag to specify whether the [`ScmpFilterAttr::CtlLog`] attribute should
    /// be enabled
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// setting the attribute, or the libseccomp API level is less than 3, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.set_ctl_log(true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_ctl_log(&mut self, state: bool) -> Result<()> {
        ensure_supported_api("set_ctl_log", 3, ScmpVersion::from((2, 4, 0)))?;
        self.set_filter_attr(ScmpFilterAttr::CtlLog, state.into())
    }

    /// Sets the state of the [`ScmpFilterAttr::CtlSsb`] attribute which will be applied on filter load.
    ///
    /// Settings this to on (`state` == `true`) disables Speculative Store Bypass mitigations for the filter.
    /// The [`ScmpFilterAttr::CtlSsb`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// Defaults to off (`state` == `false`).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `state` - A state flag to specify whether the [`ScmpFilterAttr::CtlSsb`] attribute should
    /// be enabled
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// setting the attribute, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0))).unwrap() {
    /// ctx.set_ctl_ssb(false)?;
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_ctl_ssb(&mut self, state: bool) -> Result<()> {
        ensure_supported_api("set_ctl_ssb", 4, ScmpVersion::from((2, 5, 0)))?;
        self.set_filter_attr(ScmpFilterAttr::CtlSsb, state.into())
    }

    /// Sets the [`ScmpFilterAttr::CtlOptimize`] level which will be applied on filter load.
    ///
    /// By default the libseccomp generates a set of sequential "if" statements for each rule in the filter.
    /// [`set_syscall_priority()`](ScmpFilterContext::set_syscall_priority) can be used to prioritize the
    /// order for the default cause. The binary tree optimization sorts by syscall numbers and generates
    /// consistent O(log n) filter traversal for every rule in the filter. The binary tree may be advantageous
    /// for large filters. Note that [`set_syscall_priority()`](ScmpFilterContext::set_syscall_priority) is
    /// ignored when `level` == `2`.
    /// The [`ScmpFilterAttr::CtlOptimize`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// The different optimization levels are described below:
    /// * `0` - Reserved value, not currently used.
    /// * `1` - Rules sorted by priority and complexity (DEFAULT).
    /// * `2` - Binary tree sorted by syscall number.
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `level` - The optimization level of the filter
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// setting the attribute, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0)))? {
    /// ctx.set_ctl_optimize(2)?;
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_ctl_optimize(&mut self, level: u32) -> Result<()> {
        ensure_supported_api("set_ctl_optimize", 4, ScmpVersion::from((2, 5, 0)))?;
        self.set_filter_attr(ScmpFilterAttr::CtlOptimize, level)
    }

    /// Sets the state of the [`ScmpFilterAttr::ApiSysRawRc`] attribute which will be applied on filter load.
    ///
    /// Settings this to on (`state` == `true`) means that the libseccomp should pass system error codes
    /// back to the caller instead of the default -ECANCELED.
    /// The [`ScmpFilterAttr::ApiSysRawRc`] attribute is only usable when the libseccomp API level 4 or higher
    /// is supported.
    ///
    /// Defaults to off (`state` == `false`).
    ///
    /// This function corresponds to
    /// [`seccomp_attr_set`](https://man7.org/linux/man-pages/man3/seccomp_attr_set.3.html).
    ///
    /// # Arguments
    ///
    /// * `state` - A state flag to specify whether the [`ScmpFilterAttr::ApiSysRawRc`] attribute should
    /// be enabled
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter, an issue is encountered
    /// setting the attribute, or the libseccomp API level is less than 4, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// # if check_api(4, ScmpVersion::from((2, 5, 0)))? {
    /// ctx.set_api_sysrawrc(true)?;
    /// # }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_api_sysrawrc(&mut self, state: bool) -> Result<()> {
        ensure_supported_api("set_api_sysrawrc", 4, ScmpVersion::from((2, 5, 0)))?;
        self.set_filter_attr(ScmpFilterAttr::ApiSysRawRc, state.into())
    }

    /// Outputs PFC(Pseudo Filter Code)-formatted, human-readable dump of a filter context's rules to a file.
    ///
    /// This function corresponds to
    /// [`seccomp_export_pfc`](https://man7.org/linux/man-pages/man3/seccomp_export_pfc.3.html).
    ///
    /// # Arguments
    ///
    /// * `fd` - A file descriptor to write to (must be open for writing)
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or  writing to the file fails,
    /// an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// # use std::io::{stdout};
    /// let ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.export_pfc(&mut stdout())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn export_pfc<T: AsRawFd>(&self, fd: &mut T) -> Result<()> {
        cvt(unsafe { seccomp_export_pfc(self.ctx.as_ptr(), fd.as_raw_fd()) })
    }

    /// Outputs BPF(Berkeley Packet Filter)-formatted, kernel-readable dump of a
    /// filter context's rules to a file.
    ///
    /// This function corresponds to
    /// [`seccomp_export_bpf`](https://man7.org/linux/man-pages/man3/seccomp_export_bpf.3.html).
    ///
    /// # Arguments
    ///
    /// * `fd` - A file descriptor to write to (must be open for writing)
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or  writing to the file fails,
    /// an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// # use std::io::{stdout};
    /// let ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.export_bpf(&mut stdout())?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn export_bpf<T: AsRawFd>(&self, fd: &mut T) -> Result<()> {
        cvt(unsafe { seccomp_export_bpf(self.ctx.as_ptr(), fd.as_raw_fd()) })
    }

    /// Resets a filter context, removing all its existing state.
    ///
    /// This function corresponds to
    /// [`seccomp_reset`](https://man7.org/linux/man-pages/man3/seccomp_reset.3.html).
    ///
    /// # Arguments
    ///
    /// * `action` - A new default action to be taken for syscalls which do not match
    ///
    /// # Errors
    ///
    /// If this function is called with an invalid filter or an issue is encountered
    /// resetting the filter, an error will be returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use libseccomp::*;
    /// let mut ctx = ScmpFilterContext::new_filter(ScmpAction::Allow)?;
    /// ctx.reset(ScmpAction::KillThread)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn reset(&mut self, action: ScmpAction) -> Result<()> {
        cvt(unsafe { seccomp_reset(self.ctx.as_ptr(), action.to_sys()) })
    }

    /// Gets a raw pointer of a seccomp filter.
    ///
    /// This function returns a raw pointer to the [`scmp_filter_ctx`].
    /// The caller must ensure that the filter outlives the pointer this function returns,
    /// or else it will end up pointing to garbage.
    /// You may only modify the filter referenced by the pointer with functions intended
    /// for this (the once provided by [`libseccomp_sys`] crate).
    #[must_use]
    pub fn as_ptr(&self) -> scmp_filter_ctx {
        self.ctx.as_ptr()
    }
}

impl Drop for ScmpFilterContext {
    /// Releases a filter context, freeing its memory.
    ///
    /// After calling this function, the given filter is no longer valid and cannot be used.
    ///
    /// This function corresponds to
    /// [`seccomp_release`](https://man7.org/linux/man-pages/man3/seccomp_release.3.html).
    fn drop(&mut self) {
        unsafe { seccomp_release(self.ctx.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_ptr() {
        let ctx = ScmpFilterContext::new_filter(ScmpAction::Allow).unwrap();
        assert_eq!(ctx.as_ptr(), ctx.ctx.as_ptr());
    }
}
