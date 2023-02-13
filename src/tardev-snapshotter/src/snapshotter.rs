use containerd_snapshots::{api, Info, Kind, Snapshotter, Usage};
use log::{debug, trace};
use oci_distribution::{secrets::RegistryAuth, Client, Reference, RegistryOperation};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs, fs::OpenOptions, io, io::Seek};
use tokio::sync::RwLock;
use tonic::Status;

const SNAPSHOT_REF_LABEL: &str = "containerd.io/snapshot.ref";
const TARGET_LAYER_DIGEST_LABEL: &str = "containerd.io/snapshot/cri.layer-digest";
const TARGET_REF_LABEL: &str = "containerd.io/snapshot/cri.image-ref";

struct Store {
    root: PathBuf,
}

impl Store {
    fn new(root: &Path) -> Self {
        Self { root: root.into() }
    }

    /// Creates a temporary staging directory for layers.
    fn staging_dir(&self) -> io::Result<tempfile::TempDir> {
        tempfile::tempdir_in(self.root.join("staging"))
    }

    /// Creates the snapshot file path from its name.
    ///
    /// If `write` is `true`, it also ensures that the directory exists.
    fn snapshot_path(&self, name: &str, write: bool) -> Result<PathBuf, Status> {
        let path = self.root.join("snapshots").join(name_to_hash(name));
        if write {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        Ok(path)
    }

    /// Creates the layer file path from its name.
    ///
    /// If `write` is `true`, it also ensures that the directory exists.
    fn layer_path(&self, name: &str, write: bool) -> Result<PathBuf, Status> {
        let path = self.root.join("layers").join(name_to_hash(name));
        if write {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
        }

        Ok(path)
    }

    /// Reads the information from storage for the given snapshot name.
    fn read_snapshot(&self, name: &str) -> Result<Info, Status> {
        let path = self.snapshot_path(name, false)?;
        let file = fs::File::open(path)?;
        serde_json::from_reader(file).map_err(|_| Status::unknown("unable to read snapshot"))
    }

    /// Writes to storage the given snapshot information.
    ///
    /// It fails if a snapshot with the given name already exists.
    fn write_snapshot(
        &mut self,
        kind: Kind,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<(), Status> {
        let info = Info {
            kind,
            name: key,
            parent,
            labels,
            ..Info::default()
        };
        let name = self.snapshot_path(&info.name, true)?;
        // TODO: How to specify the file mode (e.g., 0600)?
        let file = OpenOptions::new().write(true).create_new(true).open(name)?;
        serde_json::to_writer_pretty(file, &info)
            .map_err(|_| Status::internal("unable to write snapshot"))
    }

    /// Creates a new snapshot for use.
    ///
    /// It checks that the parent chain exists and that all ancestors are committed and consist of
    /// layers before writing the new snapshot.
    fn prepare_snapshot_for_use(
        &mut self,
        kind: Kind,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<Vec<api::types::Mount>, Status> {
        let mounts = self.mounts_from_snapshot(&parent)?;
        self.write_snapshot(kind, key, parent, labels)?;
        Ok(mounts)
    }

    fn mounts_from_snapshot(&self, parent: &str) -> Result<Vec<api::types::Mount>, Status> {
        // Get chain of parents.
        let mut next_parent = Some(parent.to_string());
        let mut parents = Vec::new();
        while let Some(p) = next_parent {
            let info = self.read_snapshot(&p)?;
            if info.kind != Kind::Committed {
                return Err(Status::failed_precondition(
                    "parent snapshot is not committed",
                ));
            }

            parents.push(name_to_hash(&p));

            next_parent = (!info.parent.is_empty()).then_some(info.parent);
        }

        parents.reverse();

        Ok(vec![api::types::Mount {
            r#type: "tar-overlay".to_string(),
            source: self.root.join("layers").to_string_lossy().into_owned(),
            target: String::new(),
            options: parents,
        }])
    }
}

/// The snapshotter that creates tar devices.
pub(crate) struct TarDevSnapshotter {
    store: RwLock<Store>,
}

impl TarDevSnapshotter {
    /// Creates a new instance of the snapshotter.
    ///
    /// `root` is the root directory where the snapshotter state is to be stored.
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            store: RwLock::new(Store::new(root)),
        }
    }

    /// Creates a new snapshot for an image layer.
    ///
    /// It downloads, decompresses, and creates the index for the layer before writing the new
    /// snapshot.
    async fn prepare_image_layer(
        &self,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<Vec<api::types::Mount>, Status> {
        let reference: Reference = {
            let image_ref = if let Some(r) = labels.get(TARGET_REF_LABEL) {
                r
            } else {
                return Err(Status::invalid_argument("missing target ref label"));
            };
            image_ref
                .parse()
                .map_err(|_| Status::invalid_argument("bad target ref"))?
        };

        let dir = self.store.read().await.staging_dir()?;

        {
            let digest_str = if let Some(d) = labels.get(TARGET_LAYER_DIGEST_LABEL) {
                d
            } else {
                return Err(Status::invalid_argument(
                    "missing target layer digest label",
                ));
            };

            let mut client = Client::new(Default::default());

            client
                .auth(
                    &reference,
                    &RegistryAuth::Anonymous,
                    RegistryOperation::Pull,
                )
                .await
                .map_err(|_| Status::internal("unable to authenticate"))?;

            // TODO: Eventually when we have the layer reference-count, switch to use `digest_str`
            // here.
            let mut name = dir.path().join(&key);
            name.set_extension("gz");
            trace!("Downloading to {:?}", &name);
            {
                let mut file = tokio::fs::File::create(&name).await?;
                if let Err(err) = client.pull_blob(&reference, digest_str, &mut file).await {
                    drop(file);
                    debug!("Download failed: {:?}", err);
                    let _ = fs::remove_file(&name);
                    return Err(Status::unknown("unable to pull blob"));
                }
            }

            // TODO: Decompress in stream instead of doing this.
            // Decompress data.
            trace!("Decompressing {:?}", &name);
            if !tokio::process::Command::new("gunzip")
                .arg(&name)
                .arg("-f")
                .spawn()?
                .wait()
                .await?
                .success()
            {
                let _ = fs::remove_file(&name);
                return Err(Status::unknown("unable to decompress layer"));
            }

            // TODO: Use file that is already opened once the previous TODO is fixed.
            name.set_extension("");
            trace!("Appending index to {:?}", &name);
            let mut file = OpenOptions::new().read(true).write(true).open(name)?;
            tarindex::append_index(&mut file)?;
        }

        // Move file to its final location and write the snapshot.
        {
            let from = dir.path().join(&key);
            let mut store = self.store.write().await;
            let to = store.layer_path(&key, true)?;
            trace!("Renaming from {:?} to {:?}", &from, &to);
            tokio::fs::rename(from, to).await?;
            store.write_snapshot(Kind::Committed, key, parent, labels)?;
        }

        trace!("Layer prepared");
        Err(Status::already_exists(""))
    }
}

#[tonic::async_trait]
impl Snapshotter for TarDevSnapshotter {
    type Error = Status;

    async fn stat(&self, key: String) -> Result<Info, Self::Error> {
        trace!("stat({})", key);
        self.store.read().await.read_snapshot(&key)
    }

    async fn update(
        &self,
        info: Info,
        fieldpaths: Option<Vec<String>>,
    ) -> Result<Info, Self::Error> {
        trace!("update({:?}, {:?})", info, fieldpaths);
        Err(Status::unimplemented("no support for updating snapshots"))
    }

    async fn usage(&self, key: String) -> Result<Usage, Self::Error> {
        trace!("usage({})", key);
        let store = self.store.read().await;

        let info = store.read_snapshot(&key)?;
        if info.kind != Kind::Committed {
            // Only committed snapshots consume storage.
            return Ok(Usage { inodes: 0, size: 0 });
        }

        let mut file = fs::File::open(store.layer_path(&key, false)?)?;
        let len = file.seek(io::SeekFrom::End(0))?;
        Ok(Usage {
            // TODO: Read the index "header" to determine the inode count.
            inodes: 1,
            size: len as _,
        })
    }

    async fn mounts(&self, key: String) -> Result<Vec<api::types::Mount>, Self::Error> {
        trace!("mounts({})", key);
        let store = self.store.read().await;
        let info = store.read_snapshot(&key)?;
        if info.kind != Kind::View && info.kind != Kind::Active {
            return Err(Status::failed_precondition(
                "parent snapshot is not active nor a view",
            ));
        }

        store.mounts_from_snapshot(&info.parent)
    }

    async fn prepare(
        &self,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<Vec<api::types::Mount>, Status> {
        trace!("prepare({}, {}, {:?})", key, parent, labels);

        // There are two reasons for preparing a snapshot: to build an image and to actually use it
        // as a container image. We determine the reason by the presence of the snapshot-ref label.
        if let Some(snapshot) = labels.get(SNAPSHOT_REF_LABEL) {
            self.prepare_image_layer(snapshot.to_string(), parent, labels)
                .await
        } else {
            self.store
                .write()
                .await
                .prepare_snapshot_for_use(Kind::Active, key, parent, labels)
        }
    }

    async fn view(
        &self,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<Vec<api::types::Mount>, Self::Error> {
        trace!("view({}, {}, {:?})", key, parent, labels);
        self.store
            .write()
            .await
            .prepare_snapshot_for_use(Kind::View, key, parent, labels)
    }

    async fn commit(
        &self,
        name: String,
        key: String,
        labels: HashMap<String, String>,
    ) -> Result<(), Self::Error> {
        trace!("commit({}, {}, {:?})", name, key, labels);
        Err(Status::unimplemented("no support for commiting snapshots"))
    }

    async fn remove(&self, key: String) -> Result<(), Self::Error> {
        trace!("remove({})", key);
        let store = self.store.write().await;

        // TODO: Move this to store.
        if let Ok(info) = store.read_snapshot(&key) {
            if info.kind == Kind::Committed {
                if let Some(_digest) = info.labels.get(TARGET_LAYER_DIGEST_LABEL) {
                    // Try to delete a layer. It's ok if it's not found.
                    // TODO: We need to ref-count the layer file so that we don't remove it here
                    // when the first reference goes away. For now we're using the snapshot name
                    // as the layer name, but eventually we want to use `digest`.
                    if let Ok(layer_path) = store.layer_path(&key, false) {
                        if let Err(e) = fs::remove_file(layer_path) {
                            if e.kind() != io::ErrorKind::NotFound {
                                return Err(e.into());
                            }
                        }
                    }
                }
            }
        }

        let name = store.snapshot_path(&key, false)?;
        fs::remove_file(name)?;

        Ok(())
    }

    type InfoStream = impl tokio_stream::Stream<Item = Result<Info, Self::Error>> + Send + 'static;
    async fn walk(&self) -> Result<Self::InfoStream, Self::Error> {
        trace!("walk()");
        let store = self.store.read().await;
        let snapshots_dir = store.root.join("snapshots");
        Ok(async_stream::try_stream! {
            let mut files = tokio::fs::read_dir(snapshots_dir).await?;
            while let Some(p) = files.next_entry().await? {
                if let Ok(f) = fs::File::open(p.path()) {
                    if let Ok(i) = serde_json::from_reader(f) {
                        yield i;
                    }
                }
            }
        })
    }
}

/// Converts the given name to a string representation of its sha256 hash.
fn name_to_hash(name: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(name);
    format!("{:x}", hasher.finalize())
}
