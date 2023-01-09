// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Third-party annotations - annotations defined by other projects or k8s plugins but that can
//! change Kata Containers behaviour.

/// Annotation to enable SGX.
///
/// Hardware-based isolation and memory encryption.
pub const SGX_EPC: &str = "sgx.intel.com/epc";
