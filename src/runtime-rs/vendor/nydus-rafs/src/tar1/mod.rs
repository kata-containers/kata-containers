// Copyright 2022 Ant Group. All rights reserved.
// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::iter::from_fn;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;
use std::sync::Arc;

use anyhow::{Context, Result};
use nydus_api::http::LocalFsConfig;
use storage::{
    backend::{localfs::LocalFs, BlobBackend, BlobReader},
    device::BlobInfo,
};

use self::pax::{
    OCIBlockBuilder, OCICharBuilder, OCIDirBuilder, OCIFifoBuilder, OCILinkBuilder, OCIRegBuilder,
    OCISocketBuilder, OCISymlinkBuilder, PAXExtensionSectionBuilder, PAXLinkBuilder,
    PAXSpecialSectionBuilder,
};

use crate::metadata::{RafsInode, RafsMode, RafsSuper};
use crate::RafsIoReader;

mod pax;
//>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>

trait Builder {
    fn append(&mut self, node: &dyn RafsInode, path: &Path) -> Result<()>;
}

///  A structure to convert a Rafs filesystem into an OCI compatible tarball.
pub struct OCIUnpacker {
    bootstrap: PathBuf,
    blob: Option<String>,
    output: PathBuf,
}

//<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<
impl OCIUnpacker {
    /// Create a new instance of `OCIUnpacker`.
    pub fn new(bootstrap: &str, blob: Option<&str>, output: &str) -> Result<Self> {
        Ok(OCIUnpacker {
            bootstrap: PathBuf::from(bootstrap),
            blob: blob.map(|v| v.to_string()),
            output: PathBuf::from(output),
        })
    }

    fn load_rafs(&self) -> Result<RafsSuper> {
        let bootstrap = OpenOptions::new()
            .read(true)
            .write(false)
            .open(&*self.bootstrap)
            .with_context(|| format!("fail to open bootstrap {:?}", self.bootstrap))?;
        let mut rs = RafsSuper {
            mode: RafsMode::Direct,
            validate_digest: false,
            ..Default::default()
        };

        rs.load(&mut (Box::new(bootstrap) as RafsIoReader))
            .with_context(|| format!("fail to load bootstrap {:?}", self.bootstrap))?;

        Ok(rs)
    }
}

impl Unpacker for OCIUnpacker {
    fn unpack(&self) -> Result<()> {
        debug!(
            "oci unpacker, bootstrap file: {:?}, blob file: {:?}, output file: {:?}",
            self.bootstrap, self.blob, self.output
        );

        let rafs = self.load_rafs()?;
        let mut builder = OCITarBuilderFactory::create(&rafs, self.blob.as_deref(), &self.output)?;

        for (node, path) in self.iterator(&rafs) {
            builder.append(&*node, &path)?;
        }

        Ok(())
    }
}

struct TarSection {
    header: tar::Header,
    data: Box<dyn Read>,
}

trait SectionBuilder {
    fn can_handle(&mut self, inode: &dyn RafsInode, path: &Path) -> bool;
    fn build(&self, inode: &dyn RafsInode, path: &Path) -> Result<Vec<TarSection>>;
}

struct OCITarBuilderFactory {}

impl OCITarBuilderFactory {
    fn create(
        meta: &RafsSuper,
        blob_path: Option<&str>,
        output_path: &Path,
    ) -> Result<Box<dyn Builder>> {
        let writer = Self::create_file_writer(output_path)?;
        let blob = meta.superblock.get_blob_infos().pop();
        let section_builders = Self::create_section_builders(blob, blob_path)?;
        let builder = OCITarBuilder::new(section_builders, writer);

        Ok(Box::new(builder) as Box<dyn Builder>)
    }

    fn create_file_writer(output_path: &Path) -> Result<tar::Builder<File>> {
        let builder = tar::Builder::new(
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .read(false)
                .open(output_path)
                .with_context(|| format!("fail to open output file {:?}", output_path))?,
        );

        Ok(builder)
    }

    fn create_section_builders(
        blob: Option<Arc<BlobInfo>>,
        blob_path: Option<&str>,
    ) -> Result<Vec<Box<dyn SectionBuilder>>> {
        // PAX basic builders
        let ext_builder = Rc::new(PAXExtensionSectionBuilder::new());
        let link_builder = Rc::new(PAXLinkBuilder::new(ext_builder.clone()));
        let special_builder = Rc::new(PAXSpecialSectionBuilder::new(ext_builder.clone()));

        // OCI builders
        let sock_builder = OCISocketBuilder::new();
        let hard_link_builder = OCILinkBuilder::new(link_builder.clone());
        let symlink_builder = OCISymlinkBuilder::new(link_builder);
        let dir_builder = OCIDirBuilder::new(ext_builder);
        let fifo_builder = OCIFifoBuilder::new(special_builder.clone());
        let char_builder = OCICharBuilder::new(special_builder.clone());
        let block_builder = OCIBlockBuilder::new(special_builder);
        let reg_builder = Self::create_reg_builder(blob, blob_path)?;

        // The order counts.
        let builders = vec![
            Box::new(sock_builder) as Box<dyn SectionBuilder>,
            Box::new(hard_link_builder),
            Box::new(dir_builder),
            Box::new(reg_builder),
            Box::new(symlink_builder),
            Box::new(fifo_builder),
            Box::new(char_builder),
            Box::new(block_builder),
        ];

        Ok(builders)
    }

    fn create_reg_builder(
        blob: Option<Arc<BlobInfo>>,
        blob_path: Option<&str>,
    ) -> Result<OCIRegBuilder> {
        let (reader, compressor) = match blob {
            None => (None, None),
            Some(ref blob) => {
                let reader = Self::create_blob_reader(blob_path)?;
                (Some(reader), Some(blob.compressor()))
            }
        };

        Ok(OCIRegBuilder::new(
            Rc::new(PAXExtensionSectionBuilder::new()),
            reader,
            compressor,
        ))
    }

    fn create_blob_reader(blob_path: Option<&str>) -> Result<Arc<dyn BlobReader>> {
        let blob_file = blob_path.unwrap_or_default();
        if blob_file == "" {
            return Err(anyhow!("please specify option --blob"));
        }

        let config = LocalFsConfig {
            blob_file: blob_file.to_string(),
            ..Default::default()
        };
        let config = serde_json::to_value(config)
            .with_context(|| format!("fail to create local backend config for {:?}", blob_path))?;
        let backend = LocalFs::new(config, Some("unpacker"))
            .with_context(|| format!("fail to create local backend for {:?}", blob_path))?;

        backend
            .get_reader("")
            .map_err(|err| anyhow!("fail to get reader, error {:?}", err))
    }
}

/// Structure to generate tar entries from Rafs inodes.
struct OCITarBuilder<W: Write = File> {
    writer: tar::Builder<W>,
    builders: Vec<Box<dyn SectionBuilder>>,
}

impl<W: Write> OCITarBuilder<W> {
    fn new(builders: Vec<Box<dyn SectionBuilder>>, writer: tar::Builder<W>) -> Self {
        Self { builders, writer }
    }
}

impl<W: Write> Builder for OCITarBuilder<W> {
    fn append(&mut self, inode: &dyn RafsInode, path: &Path) -> Result<()> {
        for builder in &mut self.builders {
            // Useless one, just go !!!!!
            if !builder.can_handle(inode, path) {
                continue;
            }

            for sect in builder.build(inode, path)? {
                self.writer.append(&sect.header, sect.data)?;
            }

            return Ok(());
        }

        bail!("node {:?} can not be unpacked", path)
    }
}
//>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>>
