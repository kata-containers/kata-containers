//
// Copyright 2021 The Sigstore Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Helper Structs to interact with the Sigstore TUF repository.
//!
//! The main interaction point is [`SigstoreRepository`], which fetches Rekor's
//! public key and Fulcio's certificate.
//!
//! These can later be given to [`cosign::ClientBuilder`](crate::cosign::ClientBuilder)
//! to enable Fulcio and Rekor integrations.
//!
//! # Example
//!
//! The `SigstoreRepository` instance can be created via the [`SigstoreRepository::fetch`]
//! method.
//!
//! ```rust,no_run
//! use sigstore::tuf::SigstoreRepository;
//! use sigstore::cosign;
//!
//! let repo = SigstoreRepository::fetch(None)
//!     .expect("Error while building SigstoreRepository");
//! let client = cosign::ClientBuilder::default()
//!     .with_rekor_pub_key(repo.rekor_pub_key())
//!     .with_fulcio_certs(repo.fulcio_certs())
//!     .build()
//!     .expect("Error while building cosign client");
//! ```
//!
//! The `SigstoreRepository::fetch` method can attempt to leverage local copies
//! of the Rekor and Fulcio files. Please refer to the
//! [method docs](SigstoreRepository::fetch) for more details.
//!
//! **Warning:** the `SigstoreRepository::fetch` method currently needs
//! special handling when invoked inside of an async context. Please refer to the
//! [method docs](SigstoreRepository::fetch) for more details.
//!
use std::path::Path;

mod constants;
use constants::*;

mod repository_helper;
use repository_helper::RepositoryHelper;

use super::errors::{Result, SigstoreError};

/// Securely fetches Rekor public key and Fulcio certificates from Sigstore's TUF repository
pub struct SigstoreRepository {
    rekor_pub_key: String,
    fulcio_certs: Vec<crate::registry::Certificate>,
}

impl SigstoreRepository {
    /// Fetch relevant information from the remote Sigstore TUF repository.
    ///
    /// ## Parameters
    ///
    /// * `checkout_dir`: path to a local directory where Rekor's public
    /// key and Fulcio's certificates can be found
    ///
    /// ## Behaviour
    ///
    /// This method requires network connectivity, because it will always
    /// reach out to Sigstore's TUF repository.
    ///
    /// This crates embeds a trusted copy of the `root.json` file of Sigstore's
    /// TUF repository. The `fetch` function will always connect to the online
    /// Sigstore's repository to update this embedded file to the latest version.
    /// The update process happens using the TUF protocol.
    ///
    /// When `checkout_dir` is specified, this method will look for the
    /// Fulcio and Rekor files inside of this directory. It will then compare the
    /// checksums of these local files with the ones reported inside of the
    /// TUF repository metadata.
    ///
    /// If the files are not found, or if their local checksums do not match
    /// with the ones reported by TUF's metdata, the files are then downloaded
    /// from the TUF repository and then written to the local filesystem.
    ///
    /// When `checkout_dir` is `None`, the `fetch` method will always fetch the
    /// Fulcio and Rekor files from the remote TUF repository and keep them
    /// in memory.
    ///
    /// ## Usage inside of async code
    ///
    /// **Warning:** this method needs special handling when invoked from
    /// an async function because it peforms blocking operations.
    ///
    /// If needed, this can be solved in that way:
    ///
    /// ```rust,no_run
    /// use tokio::task::spawn_blocking;
    /// use sigstore::tuf::SigstoreRepository;
    ///
    /// async fn my_async_function() {
    ///    // ... your code
    ///
    ///    let repo: sigstore::errors::Result<SigstoreRepository> = spawn_blocking(||
    ///      sigstore::tuf::SigstoreRepository::fetch(None)
    ///    )
    ///    .await
    ///    .expect("Error spawning blocking task");
    ///
    ///    // handle the case of `repo` being an `Err`
    ///    // ... your code
    /// }
    /// ```
    ///
    /// This of course has a performance hit when used inside of an async function.
    pub fn fetch(checkout_dir: Option<&Path>) -> Result<Self> {
        let metadata_base = url::Url::parse(SIGSTORE_METADATA_BASE).map_err(|_| {
            SigstoreError::UnexpectedError(String::from("Cannot convert metadata_base to URL"))
        })?;
        let target_base = url::Url::parse(SIGSTORE_TARGET_BASE).map_err(|_| {
            SigstoreError::UnexpectedError(String::from("Cannot convert target_base to URL"))
        })?;

        let repository_helper = RepositoryHelper::new(
            SIGSTORE_ROOT.as_bytes(),
            metadata_base,
            target_base,
            checkout_dir,
        )?;

        let fulcio_certs = repository_helper.fulcio_certs()?;

        let rekor_pub_key = repository_helper.rekor_pub_key().map(|data| {
            String::from_utf8(data).map_err(|e| {
                SigstoreError::UnexpectedError(format!(
                    "Cannot parse Rekor's public key obtained from TUF repository: {}",
                    e
                ))
            })
        })??;

        Ok(SigstoreRepository {
            rekor_pub_key,
            fulcio_certs,
        })
    }

    /// Rekor public key
    pub fn rekor_pub_key(&self) -> &str {
        &self.rekor_pub_key
    }

    /// Fulcio certificate
    pub fn fulcio_certs(&self) -> &[crate::registry::Certificate] {
        &self.fulcio_certs
    }
}
