// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

//! # Payload format for a signature
//! If new payload formats appear in the future,
//! they can be abstracted into a new trait
//! here.
//!
//! Now support the following payload formats:
//! * [SimpleSigning](https://github.com/containers/image/blob/main/docs/containers-signature.5.md#json-data-format)

pub mod simple_signing;
