use crate::error::{self, Result};
use crate::fetch::{fetch_max_size, fetch_sha256};
use crate::schema::{RoleType, Target};
use crate::{encode_filename, Prefix, Repository, TargetName};
use snafu::{OptionExt, ResultExt};
use std::io::{Read, Write};
use std::path::Path;

impl Repository {
    /// Cache an entire or partial repository to disk, including all required metadata.
    /// The cached repo will be local, using filesystem paths.
    ///
    /// * `metadata_outdir` is the directory where cached metadata files will be saved.
    /// * `targets_outdir` is the directory where cached targets files will be saved.
    /// * `targets_subset` is the list of targets to include in the cached repo. If no subset is
    /// specified (`None`), then *all* targets are included in the cache.
    /// * `cache_root_chain` specifies whether or not we will cache all versions of `root.json`.
    pub fn cache<P1, P2, S>(
        &self,
        metadata_outdir: P1,
        targets_outdir: P2,
        targets_subset: Option<&[S]>,
        cache_root_chain: bool,
    ) -> Result<()>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
        S: AsRef<str>,
    {
        // Create the output directories if the do not exist.
        std::fs::create_dir_all(metadata_outdir.as_ref()).context(
            error::CacheDirectoryCreateSnafu {
                path: metadata_outdir.as_ref(),
            },
        )?;
        std::fs::create_dir_all(targets_outdir.as_ref()).context(
            error::CacheDirectoryCreateSnafu {
                path: targets_outdir.as_ref(),
            },
        )?;

        // Fetch targets and save them to the outdir
        if let Some(target_list) = targets_subset {
            for raw_name in target_list.iter() {
                let target_name = TargetName::new(raw_name.as_ref())?;
                self.cache_target(&targets_outdir, &target_name)?;
            }
        } else {
            let targets = &self.targets.signed.targets_map();
            for target_name in targets.keys() {
                self.cache_target(&targets_outdir, target_name)?;
            }
        }

        // Cache all metadata
        self.cache_metadata_impl(&metadata_outdir)?;

        if cache_root_chain {
            self.cache_root_chain(&metadata_outdir)?;
        }
        Ok(())
    }

    /// Cache only a repository's metadata files (snapshot, targets, timestamp), including any
    /// delegated targets metadata.  The cached files will be saved to the local filesystem.
    ///
    /// * `metadata_outdir` is the directory where cached metadata files will be saved.
    /// * `cache_root_chain` specifies whether or not we will cache all versions of `root.json`.
    pub fn cache_metadata<P>(&self, metadata_outdir: P, cache_root_chain: bool) -> Result<()>
    where
        P: AsRef<Path>,
    {
        // Create the output directory if it does not exist.
        std::fs::create_dir_all(metadata_outdir.as_ref()).context(
            error::CacheDirectoryCreateSnafu {
                path: metadata_outdir.as_ref(),
            },
        )?;

        self.cache_metadata_impl(&metadata_outdir)?;

        if cache_root_chain {
            self.cache_root_chain(metadata_outdir)?;
        }
        Ok(())
    }

    /// Cache repository metadata files, including delegated targets metadata
    fn cache_metadata_impl<P>(&self, metadata_outdir: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        self.cache_file_from_transport(
            self.snapshot_filename().as_str(),
            self.max_snapshot_size()?,
            "timestamp.json",
            &metadata_outdir,
        )?;
        self.cache_file_from_transport(
            self.targets_filename().as_str(),
            self.limits.max_targets_size,
            "max_targets_size argument",
            &metadata_outdir,
        )?;
        self.cache_file_from_transport(
            "timestamp.json",
            self.limits.max_timestamp_size,
            "max_timestamp_size argument",
            &metadata_outdir,
        )?;

        for name in self.targets.signed.role_names() {
            if let Some(filename) = self.delegated_filename(name) {
                self.cache_file_from_transport(
                    filename.as_str(),
                    self.limits.max_targets_size,
                    "max_targets_size argument",
                    &metadata_outdir,
                )?;
            }
        }

        Ok(())
    }

    /// Cache all versions of root.json less than or equal to the current version.
    fn cache_root_chain<P>(&self, outdir: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        for ver in (1..=self.root.signed.version.get()).rev() {
            let root_json_filename = format!("{}.root.json", ver);
            self.cache_file_from_transport(
                root_json_filename.as_str(),
                self.limits.max_root_size,
                "max_root_size argument",
                &outdir,
            )?;
        }
        Ok(())
    }

    /// Prepends the version number to the snapshot.json filename if using consistent snapshot mode.
    fn snapshot_filename(&self) -> String {
        if self.root.signed.consistent_snapshot {
            format!("{}.snapshot.json", self.snapshot.signed.version)
        } else {
            "snapshot.json".to_owned()
        }
    }

    /// Prepends the version number to the targets.json filename if using consistent snapshot mode.
    fn targets_filename(&self) -> String {
        if self.root.signed.consistent_snapshot {
            format!("{}.targets.json", self.targets.signed.version)
        } else {
            "targets.json".to_owned()
        }
    }

    /// Prepends the version number to the role.json filename if using consistent snapshot mode.
    fn delegated_filename(&self, name: &str) -> Option<String> {
        if self.root.signed.consistent_snapshot {
            Some(format!(
                "{}.{}.json",
                self.snapshot
                    .signed
                    .meta
                    .get(&format!("{}.json", name))?
                    .version,
                encode_filename(name)
            ))
        } else {
            Some(format!("{}.json", encode_filename(name)))
        }
    }

    /// Copies a file using `Transport` to `outdir`.
    fn cache_file_from_transport<P: AsRef<Path>>(
        &self,
        filename: &str,
        max_size: u64,
        max_size_specifier: &'static str,
        outdir: P,
    ) -> Result<()> {
        let mut read = fetch_max_size(
            self.transport.as_ref(),
            self.metadata_base_url
                .join(filename)
                .context(error::JoinUrlSnafu {
                    path: filename,
                    url: self.metadata_base_url.clone(),
                })?,
            max_size,
            max_size_specifier,
        )?;
        let outpath = outdir.as_ref().join(&filename);
        let mut file = std::fs::File::create(&outpath).context(error::CacheFileWriteSnafu {
            path: outpath.clone(),
        })?;
        let mut root_file_data = Vec::new();
        read.read_to_end(&mut root_file_data)
            .context(error::CacheFileReadSnafu {
                url: self.metadata_base_url.clone(),
            })?;
        file.write_all(&root_file_data)
            .context(error::CacheFileWriteSnafu { path: outpath })
    }

    /// Saves a signed target to the specified `outdir`. Retains the digest-prepended filename if
    /// consistent snapshots are used.
    fn cache_target<P: AsRef<Path>>(&self, outdir: P, name: &TargetName) -> Result<()> {
        self.save_target(
            name,
            outdir,
            if self.consistent_snapshot {
                Prefix::Digest
            } else {
                Prefix::None
            },
        )
    }

    /// Gets the max size of the snapshot.json file as specified by the timestamp file.
    fn max_snapshot_size(&self) -> Result<u64> {
        let snapshot_meta =
            self.timestamp()
                .signed
                .meta
                .get("snapshot.json")
                .context(error::MetaMissingSnafu {
                    file: "snapshot.json",
                    role: RoleType::Timestamp,
                })?;
        Ok(snapshot_meta.length)
    }

    /// Prepends the target digest to the name if using consistent snapshots. Returns both the
    /// digest and the filename.
    pub(crate) fn target_digest_and_filename(
        &self,
        target: &Target,
        name: &TargetName,
    ) -> (Vec<u8>, String) {
        let sha256 = &target.hashes.sha256.clone().into_vec();
        if self.consistent_snapshot {
            (
                sha256.clone(),
                format!("{}.{}", hex::encode(sha256), name.resolved()),
            )
        } else {
            (sha256.clone(), name.resolved().to_owned())
        }
    }

    /// Fetches the signed target using `Transport`. Aborts with error if the fetched target is
    /// larger than its signed size.
    pub(crate) fn fetch_target(
        &self,
        target: &Target,
        digest: &[u8],
        filename: &str,
    ) -> Result<impl Read> {
        fetch_sha256(
            self.transport.as_ref(),
            self.targets_base_url
                .join(filename)
                .context(error::JoinUrlSnafu {
                    path: filename,
                    url: self.targets_base_url.clone(),
                })?,
            target.length,
            "targets.json",
            digest,
        )
    }
}
