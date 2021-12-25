// Copyright (c) 2021 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! Third-party annotations - annotations defined by other projects or k8s plugins but that can
//! change Kata Containers behaviour.

/// Annotation to enable SGX.
///
/// Hardware-based isolation and memory encryption.
// Supported suffixes are: Ki | Mi | Gi | Ti | Pi | Ei . For example: 4Mi
// For more information about supported suffixes see https://physics.nist.gov/cuu/Units/binary.html
pub const SGXEPC: &str = "sgx.intel.com/epc";
