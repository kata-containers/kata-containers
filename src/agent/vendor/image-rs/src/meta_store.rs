use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fs::File;
use std::path::Path;

use crate::image::{ImageMeta, LayerMeta};

pub const METAFILE: &str = "meta_store.json";

/// `image-rs` container metadata storage database.
#[derive(Clone, Default, Deserialize, Debug)]
pub struct MetaStore {
    // image_db holds map of image ID with image data.
    pub image_db: HashMap<String, ImageMeta>,

    // layer_db holds map of layer digest with layer meta.
    pub layer_db: HashMap<String, LayerMeta>,

    // snapshot_db holds map of snapshot with work dir index.
    pub snapshot_db: HashMap<String, usize>,
}

impl TryFrom<&Path> for MetaStore {
    /// load `MetaStore` from a local file
    type Error = anyhow::Error;
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        let file = File::open(path)
            .map_err(|e| anyhow!("failed to open metastore file {}", e.to_string()))?;
        serde_json::from_reader::<File, MetaStore>(file)
            .map_err(|e| anyhow!("failed to parse metastore file {}", e.to_string()))
    }
}
