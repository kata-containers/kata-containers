use base64::prelude::{Engine, BASE64_STANDARD};
use containerd_client::{services::v1::ReadContentRequest, tonic::Request, with_namespace, Client};
use containerd_snapshots::{api, Info, Kind, Snapshotter, Usage};
use log::{debug, info, trace};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::{collections::HashMap, fs, fs::OpenOptions, io, io::Seek, os::unix::ffi::OsStrExt};
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tonic::Status;

const ROOT_HASH_LABEL: &str = "io.katacontainers.dm-verity.root-hash";
const TARGET_LAYER_DIGEST_LABEL: &str = "containerd.io/snapshot/cri.layer-digest";

struct Store {
    root: PathBuf,
}

impl Store {
    fn new(root: &Path) -> Self {
        Self { root: root.into() }
    }

    /// Creates the name of the directory that containerd can use to extract a layer into.
    fn extract_dir(&self, name: &str) -> PathBuf {
        self.root.join("staging").join(name_to_hash(name))
    }

    /// Creates a directory that containerd can use to extract a layer into.
    ///
    /// It's a temporary directory that will be thrown away by the snapshotter.
    fn extract_dir_to_write(&self, name: &str) -> io::Result<PathBuf> {
        let path = self.extract_dir(name);
        fs::create_dir_all(&path)?;
        Ok(path)
    }

    /// Creates a temporary staging directory for layers.
    fn staging_dir(&self) -> io::Result<tempfile::TempDir> {
        let path = self.root.join("staging");
        fs::create_dir_all(&path)?;
        tempfile::tempdir_in(path)
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
    fn layer_path(&self, name: &str) -> PathBuf {
        self.root.join("layers").join(name_to_hash(name))
    }

    /// Creates the layer file path from its name and ensures that the directory exists.
    fn layer_path_to_write(&self, name: &str) -> Result<PathBuf, Status> {
        let path = self.layer_path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
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
        const PREFIX: &str = "io.katacontainers.fs-opt";

        // Get chain of layers.
        let mut next_parent = Some(parent.to_string());
        let mut layers = Vec::new();
        let mut opts = vec![format!(
            "{PREFIX}.layer-src-prefix={}",
            self.root.join("layers").to_string_lossy()
        )];
        while let Some(p) = next_parent {
            let info = self.read_snapshot(&p)?;
            if info.kind != Kind::Committed {
                return Err(Status::failed_precondition(
                    "parent snapshot is not committed",
                ));
            }

            let root_hash = if let Some(rh) = info.labels.get(ROOT_HASH_LABEL) {
                rh
            } else {
                return Err(Status::failed_precondition(
                    "parent snapshot has no root hash stored",
                ));
            };

            let name = name_to_hash(&p);
            let layer_info = format!(
                "{name},tar,ro,{PREFIX}.block_device=file,{PREFIX}.is-layer,{PREFIX}.root-hash={root_hash}");
            layers.push(name);

            opts.push(format!(
                "{PREFIX}.layer={}",
                BASE64_STANDARD.encode(layer_info.as_bytes())
            ));

            next_parent = (!info.parent.is_empty()).then_some(info.parent);
        }

        opts.push(format!("{PREFIX}.overlay-rw"));
        opts.push(format!("lowerdir={}", layers.join(":")));

        Ok(vec![api::types::Mount {
            r#type: "fuse3.kata-overlay".into(),
            source: "/".into(),
            target: String::new(),
            options: opts,
        }])
    }
}

/// The snapshotter that creates tar devices.
pub(crate) struct TarDevSnapshotter {
    store: RwLock<Store>,
    containerd_path: String,
    containerd_client: RwLock<Option<Client>>,
}

impl TarDevSnapshotter {
    /// Creates a new instance of the snapshotter.
    ///
    /// `root` is the root directory where the snapshotter state is to be stored.
    pub(crate) fn new(root: &Path, containerd_path: String) -> Self {
        Self {
            containerd_path,
            store: RwLock::new(Store::new(root)),
            containerd_client: RwLock::new(None),
        }
    }

    async fn prepare_unpack_dir(
        &self,
        key: String,
        parent: String,
        labels: HashMap<String, String>,
    ) -> Result<Vec<api::types::Mount>, Status> {
        let extract_dir;
        {
            let mut store = self.store.write().await;
            extract_dir = store.extract_dir_to_write(&key)?;
            store.write_snapshot(Kind::Active, key, parent, labels)?;
        }
        Ok(vec![api::types::Mount {
            r#type: "bind".into(),
            source: extract_dir.to_string_lossy().into(),
            target: String::new(),
            options: vec!["bind".into()],
        }])
    }

    async fn get_layer_image(&self, fname: &PathBuf, digest: &str) -> Result<(), Status> {
        let mut file = tokio::fs::File::create(fname).await?;
        let req = ReadContentRequest {
            digest: digest.to_string(),
            offset: 0,
            size: 0,
        };
        let req = with_namespace!(req, "k8s.io");

        loop {
            let guard = self.containerd_client.read().await;
            let Some(client) = &*guard else {
                drop(guard);
                info!("Connecting to containerd at {}", self.containerd_path);
                let c = Client::from_path(&self.containerd_path)
                    .await
                    .map_err(|_| Status::unknown("unable to connect to containerd"))?;
                *self.containerd_client.write().await = Some(c);
                continue;
            };
            let mut c = client.content();
            let resp = c.read(req).await?;
            let mut stream = resp.into_inner();
            while let Some(chunk) = stream.message().await? {
                if chunk.offset < 0 {
                    debug!("Containerd reported a negative offset: {}", chunk.offset);
                    return Err(Status::invalid_argument("negative offset"));
                }
                file.seek(io::SeekFrom::Start(chunk.offset as u64)).await?;
                file.write_all(&chunk.data).await?;
            }

            return Ok(());
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
        mut labels: HashMap<String, String>,
    ) -> Result<(), Status> {
        let dir = self.store.read().await.staging_dir()?;

        {
            let Some(digest_str) = labels.get(TARGET_LAYER_DIGEST_LABEL) else {
                return Err(Status::invalid_argument(
                    "missing target layer digest label",
                ));
            };

            let name = dir.path().join(name_to_hash(&key));
            let mut gzname = name.clone();
            gzname.set_extension("gz");
            trace!("Fetching layer image to {:?}", &gzname);
            self.get_layer_image(&gzname, digest_str).await?;

            // TODO: Decompress in stream instead of reopening.
            // Decompress data.
            trace!("Decompressing {:?} to {:?}", &gzname, &name);
            let root_hash = tokio::task::spawn_blocking(move || -> io::Result<_> {
                let compressed = fs::File::open(&gzname)?;
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&name)?;
                let mut gz_decoder = flate2::read::GzDecoder::new(compressed);
                std::io::copy(&mut gz_decoder, &mut file)?;

                trace!("Appending index to {:?}", &name);
                file.rewind()?;
                tarindex::append_index(&mut file)?;

                trace!("Appending dm-verity tree to {:?}", &name);
                let root_hash = verity::append_tree::<Sha256>(&mut file)?;

                trace!("Root hash for {:?} is {:x}", &name, root_hash);
                Ok(root_hash)
            })
            .await
            .map_err(|_| Status::unknown("error in worker task"))??;

            // Store a label with the root hash so that we can recall it later when mounting.
            labels.insert(ROOT_HASH_LABEL.into(), format!("{:x}", root_hash));
        }

        // Move file to its final location and write the snapshot.
        {
            let from = dir.path().join(name_to_hash(&key));
            let mut store = self.store.write().await;
            let to = store.layer_path_to_write(&key)?;
            trace!("Renaming from {:?} to {:?}", &from, &to);
            tokio::fs::rename(from, to).await?;
            store.write_snapshot(Kind::Committed, key, parent, labels)?;
        }

        trace!("Layer prepared");
        Ok(())
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

        let mut file = tokio::fs::File::open(store.layer_path(&key)).await?;
        let len = file.seek(io::SeekFrom::End(0)).await?;
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
                "snapshot is not active nor a view",
            ));
        }

        if info.labels.get(TARGET_LAYER_DIGEST_LABEL).is_some() {
            let extract_dir = store.extract_dir(&key);
            Ok(vec![api::types::Mount {
                r#type: "bind".into(),
                source: extract_dir.to_string_lossy().into(),
                target: String::new(),
                options: Vec::new(),
            }])
        } else {
            store.mounts_from_snapshot(&info.parent)
        }
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
        if labels.get(TARGET_LAYER_DIGEST_LABEL).is_some() {
            self.prepare_unpack_dir(key, parent, labels).await
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

        let info;
        {
            let store = self.store.write().await;
            info = store.read_snapshot(&key)?;
            if info.kind != Kind::Active {
                return Err(Status::failed_precondition("snapshot is not active"));
            }
        }

        if info.labels.get(TARGET_LAYER_DIGEST_LABEL).is_some() {
            self.prepare_image_layer(name, info.parent, labels).await
        } else {
            Err(Status::unimplemented(
                "no support for commiting arbitrary snapshots",
            ))
        }
    }

    async fn remove(&self, key: String) -> Result<(), Self::Error> {
        trace!("remove({})", key);
        let store = self.store.write().await;

        // TODO: Move this to store.
        if let Ok(info) = store.read_snapshot(&key) {
            match info.kind {
                Kind::Committed => {
                    if info.labels.get(TARGET_LAYER_DIGEST_LABEL).is_some() {
                        // Try to delete a layer. It's ok if it's not found.
                        if let Err(e) = fs::remove_file(store.layer_path(&key)) {
                            if e.kind() != io::ErrorKind::NotFound {
                                return Err(e.into());
                            }
                        }
                    }
                }
                Kind::Active => {
                    if let Err(e) = tokio::fs::remove_dir_all(store.extract_dir(&key)).await {
                        if e.kind() != io::ErrorKind::NotFound {
                            return Err(e.into());
                        }
                    }
                }
                _ => {}
            }
        }

        let name = store.snapshot_path(&key, false)?;
        fs::remove_file(name)?;

        Ok(())
    }

    type InfoStream = impl tokio_stream::Stream<Item = Result<Info, Self::Error>> + Send + 'static;
    async fn list(&self, _: String, _: Vec<String>) -> Result<Self::InfoStream, Self::Error> {
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
    let path = Path::new(name);
    let mut hasher = Sha256::new();
    match path.file_name() {
        Some(n) => hasher.update(n.as_bytes()),
        None => hasher.update(name),
    }
    format!("{:x}", hasher.finalize())
}
