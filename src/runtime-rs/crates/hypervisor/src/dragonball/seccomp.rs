// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use seccompiler::{BpfProgram, SeccompAction, SeccompFilter};
use std::convert::TryInto;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ThreadType {
    Vcpu,
    Vmm,
}

pub fn get_seccomp_filter(thread_type: &ThreadType) -> BpfProgram {
    let rules = match thread_type {
        ThreadType::Vcpu => get_vcpu_seccomp_rules(),
        ThreadType::Vmm => get_vmm_seccomp_rules(),
    };
    SeccompFilter::new(
        rules.into_iter().collect(),
        // TODO: modify the action after determining the action needed for dragonball
        SeccompAction::Allow,
        SeccompAction::Allow,
        std::env::consts::ARCH.try_into().unwrap(),
    )
    .and_then(|f| f.try_into())
    .unwrap_or_default()
}

pub fn get_vcpu_seccomp_rules() -> Vec<(i64, Vec<seccompiler::SeccompRule>)> {
    // TODO: add vcpu seccomp rules
    vec![]
}

pub fn get_vmm_seccomp_rules() -> Vec<(i64, Vec<seccompiler::SeccompRule>)> {
    // TODO: add vmm seccomp rules
    vec![]
}
