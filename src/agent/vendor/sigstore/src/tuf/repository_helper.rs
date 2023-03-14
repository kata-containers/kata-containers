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

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use tough::{RepositoryLoader, TargetName};
use url::Url;

use super::{
    super::errors::{Result, SigstoreError},
    constants::{SIGSTORE_FULCIO_CERT_TARGET_REGEX, SIGSTORE_REKOR_PUB_KEY_TARGET},
};

pub(crate) struct RepositoryHelper {
    repository: tough::Repository,
    checkout_dir: Option<PathBuf>,
}

impl RepositoryHelper {
    pub(crate) fn new<R>(
        root: R,
        metadata_base: Url,
        target_base: Url,
        checkout_dir: Option<&Path>,
    ) -> Result<Self>
    where
        R: Read,
    {
        let repository = RepositoryLoader::new(root, metadata_base, target_base)
            .expiration_enforcement(tough::ExpirationEnforcement::Safe)
            .load()?;

        Ok(Self {
            repository,
            checkout_dir: checkout_dir.map(|s| s.to_owned()),
        })
    }

    /// Fetch Fulcio certificates from the given TUF repository or reuse
    /// the local cache if its contents are not outdated.
    ///
    /// The contents of the local cache are updated when they are outdated.
    pub(crate) fn fulcio_certs(&self) -> Result<Vec<crate::registry::Certificate>> {
        let fulcio_target_names = self.fulcio_cert_target_names();
        let mut certs = vec![];

        for fulcio_target_name in &fulcio_target_names {
            let local_fulcio_path = self
                .checkout_dir
                .as_ref()
                .map(|d| Path::new(d).join(fulcio_target_name.raw()));

            let cert_data = fetch_target_or_reuse_local_cache(
                &self.repository,
                fulcio_target_name,
                local_fulcio_path.as_ref(),
            )?;
            certs.push(crate::registry::Certificate {
                data: cert_data,
                encoding: crate::registry::CertificateEncoding::Pem,
            });
        }
        Ok(certs)
    }

    fn fulcio_cert_target_names(&self) -> Vec<TargetName> {
        self.repository
            .targets()
            .signed
            .targets_iter()
            .filter_map(|(target_name, _target)| {
                if SIGSTORE_FULCIO_CERT_TARGET_REGEX.is_match(target_name.raw()) {
                    Some(target_name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Fetch Rekor public key from the given TUF repository or reuse
    /// the local cache if it's not outdated.
    ///
    /// The contents of the local cache are updated when they are outdated.
    pub(crate) fn rekor_pub_key(&self) -> Result<Vec<u8>> {
        let rekor_target_name = TargetName::new(SIGSTORE_REKOR_PUB_KEY_TARGET)?;

        let local_rekor_path = self
            .checkout_dir
            .as_ref()
            .map(|d| Path::new(d).join(SIGSTORE_REKOR_PUB_KEY_TARGET));

        fetch_target_or_reuse_local_cache(
            &self.repository,
            &rekor_target_name,
            local_rekor_path.as_ref(),
        )
    }
}

/// Download a file stored inside of a TUF repository, try to reuse a local
/// cache when possible.
///
/// * `repository`: TUF repository holding the file
/// * `target`: TUF representation of the file to be downloaded
/// * `local_file`: location where the file should be downloaded
///
/// This function will reuse the local copy of the file if contents
/// didn't change.
/// This check is done by comparing the digest of the local file, if found,
/// with the digest reported inside of the TUF repository metadata.
///
/// **Note well:** the `local_file` is updated whenever its contents are
/// outdated.
fn fetch_target_or_reuse_local_cache(
    repository: &tough::Repository,
    target_name: &TargetName,
    local_file: Option<&PathBuf>,
) -> Result<Vec<u8>> {
    let (local_file_outdated, local_file_contents) = if let Some(path) = local_file {
        is_local_file_outdated(repository, target_name, path)
    } else {
        Ok((true, None))
    }?;

    let data = if local_file_outdated {
        let data = fetch_target(repository, target_name)?;
        if let Some(path) = local_file {
            // update the local file to have latest data from the TUF repo
            fs::write(path, data.clone())?;
        }
        data
    } else {
        local_file_contents
            .expect("local file contents to not be 'None'")
            .as_bytes()
            .to_owned()
    };

    Ok(data)
}

/// Download a file from a TUF repository
fn fetch_target(repository: &tough::Repository, target_name: &TargetName) -> Result<Vec<u8>> {
    let data: Vec<u8>;
    match repository.read_target(target_name)? {
        None => Err(SigstoreError::TufTargetNotFoundError(
            target_name.raw().to_string(),
        )),
        Some(reader) => {
            data = read_to_end(reader)?;
            Ok(data)
        }
    }
}

/// Compares the checksum of a local file, with the digest reported inside of
/// TUF repository metadata
fn is_local_file_outdated(
    repository: &tough::Repository,
    target_name: &TargetName,
    local_file: &Path,
) -> Result<(bool, Option<String>)> {
    let target = repository
        .targets()
        .signed
        .targets
        .get(target_name)
        .ok_or_else(|| SigstoreError::TufTargetNotFoundError(target_name.raw().to_string()))?;

    if local_file.exists() {
        let data = fs::read_to_string(local_file)?;
        let local_checksum = Sha256::digest(data.clone());
        let expected_digest: Vec<u8> = target.hashes.sha256.to_vec();

        if local_checksum.as_slice() == expected_digest.as_slice() {
            // local data is not outdated
            Ok((false, Some(data)))
        } else {
            Ok((true, None))
        }
    } else {
        Ok((true, None))
    }
}

/// Gets the goods from a read and makes a Vec
fn read_to_end<R: Read>(mut reader: R) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    reader.read_to_end(&mut v)?;
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::super::constants::*;
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Returns the path to our test data directory
    fn test_data() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
    }

    fn local_tuf_repo() -> Result<tough::Repository> {
        let metadata_base_path = test_data().join("repository");
        let targets_base_path = metadata_base_path.join("targets");

        let metadata_base_url = format!(
            "file://{}",
            metadata_base_path
                .to_str()
                .ok_or_else(|| SigstoreError::UnexpectedError(String::from(
                    "Cannot convert metadata_base_path into a str"
                )))?
        );
        let metadata_base_url = url::Url::parse(&metadata_base_url).map_err(|_| {
            SigstoreError::UnexpectedError(String::from(
                "Cannot convert metadata_base_url into a URL",
            ))
        })?;

        let target_base_url = format!(
            "file://{}",
            targets_base_path
                .to_str()
                .ok_or_else(|| SigstoreError::UnexpectedError(String::from(
                    "Cannot convert targets_base_path into a str"
                )))?
        );
        let target_base_url = url::Url::parse(&target_base_url).map_err(|_| {
            SigstoreError::UnexpectedError(String::from(
                "Cannot convert targets_base_url into a URL",
            ))
        })?;
        // It's fine to ignore timestamp.json expiration inside of test env
        let repo =
            RepositoryLoader::new(SIGSTORE_ROOT.as_bytes(), metadata_base_url, target_base_url)
                .expiration_enforcement(tough::ExpirationEnforcement::Unsafe)
                .load()?;
        Ok(repo)
    }

    #[test]
    fn get_files_without_using_local_cache() {
        let repository = local_tuf_repo().expect("Local TUF repo should not fail");
        let helper = RepositoryHelper {
            repository,
            checkout_dir: None,
        };

        let mut actual = helper.fulcio_certs().expect("fulcio certs cannot be read");
        actual.sort();
        let mut expected: Vec<crate::registry::Certificate> =
            vec!["fulcio.crt.pem", "fulcio_v1.crt.pem"]
                .iter()
                .map(|filename| {
                    let data = fs::read(
                        test_data()
                            .join("repository")
                            .join("targets")
                            .join(filename),
                    )
                    .expect(format!("cannot read {} from test data", filename).as_str());
                    crate::registry::Certificate {
                        data,
                        encoding: crate::registry::CertificateEncoding::Pem,
                    }
                })
                .collect();
        expected.sort();

        assert_eq!(
            actual, expected,
            "The fulcio cert read from the TUF repository is not what was expected"
        );

        let actual = helper.rekor_pub_key().expect("rekor key cannot be read");
        let expected = fs::read(
            test_data()
                .join("repository")
                .join("targets")
                .join("rekor.pub"),
        )
        .expect("cannot read rekor key from test data");

        assert_eq!(
            actual, expected,
            "The rekor key read from the TUF repository is not what was expected"
        );
    }

    #[test]
    fn download_files_to_local_cache() {
        let cache_dir = TempDir::new().expect("Cannot create temp cache dir");

        let repository = local_tuf_repo().expect("Local TUF repo should not fail");
        let helper = RepositoryHelper {
            repository,
            checkout_dir: Some(cache_dir.path().to_path_buf()),
        };

        let mut actual = helper.fulcio_certs().expect("fulcio certs cannot be read");
        actual.sort();
        let mut expected: Vec<crate::registry::Certificate> =
            vec!["fulcio.crt.pem", "fulcio_v1.crt.pem"]
                .iter()
                .map(|filename| {
                    let data = fs::read(
                        test_data()
                            .join("repository")
                            .join("targets")
                            .join(filename),
                    )
                    .expect(format!("cannot read {} from test data", filename).as_str());
                    crate::registry::Certificate {
                        data,
                        encoding: crate::registry::CertificateEncoding::Pem,
                    }
                })
                .collect();
        expected.sort();

        assert_eq!(
            actual, expected,
            "The fulcio cert read from the cache dir is not what was expected"
        );

        let expected = helper.rekor_pub_key().expect("rekor key cannot be read");
        let actual = fs::read(cache_dir.path().join("rekor.pub"))
            .expect("cannot read rekor key from cache dir");

        assert_eq!(
            actual, expected,
            "The rekor key read from the cache dir is not what was expected"
        );
    }

    #[test]
    fn update_local_cache() {
        let cache_dir = TempDir::new().expect("Cannot create temp cache dir");

        // put some outdated files inside of the cache
        for filename in vec!["fulcio.crt.pem", "fulcio_v1.crt.pem"] {
            fs::write(cache_dir.path().join(filename), b"fake fulcio")
                .expect("Cannot write file to cache dir");
        }
        fs::write(
            cache_dir.path().join(SIGSTORE_REKOR_PUB_KEY_TARGET),
            b"fake rekor",
        )
        .expect("Cannot write file to cache dir");

        let repository = local_tuf_repo().expect("Local TUF repo should not fail");
        let helper = RepositoryHelper {
            repository,
            checkout_dir: Some(cache_dir.path().to_path_buf()),
        };

        let mut actual = helper.fulcio_certs().expect("fulcio certs cannot be read");
        actual.sort();
        let mut expected: Vec<crate::registry::Certificate> =
            vec!["fulcio.crt.pem", "fulcio_v1.crt.pem"]
                .iter()
                .map(|filename| {
                    let data = fs::read(
                        test_data()
                            .join("repository")
                            .join("targets")
                            .join(filename),
                    )
                    .expect(format!("cannot read {} from test data", filename).as_str());
                    crate::registry::Certificate {
                        data,
                        encoding: crate::registry::CertificateEncoding::Pem,
                    }
                })
                .collect();
        expected.sort();

        assert_eq!(
            actual, expected,
            "The fulcio cert read from the TUF repository is not what was expected"
        );

        let expected = helper.rekor_pub_key().expect("rekor key cannot be read");
        let actual = fs::read(cache_dir.path().join("rekor.pub"))
            .expect("cannot read rekor key from cache dir");

        assert_eq!(
            actual, expected,
            "The rekor key read from the cache dir is not what was expected"
        );
    }
}
