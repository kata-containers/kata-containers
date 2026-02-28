mod initdata;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use slog::{info, o, warn, Drain, Logger};

use crate::initdata::{locate_device_concurrently, read_initdata};
use kata_types::initdata::InitData;

const MEASURED_CFG_DIR: &str = "/run/measured-cfg";

#[derive(Debug)]
struct InitDataProcessor {
    device_path: PathBuf,
    config_path: PathBuf,
    logger: Logger,
}

impl InitDataProcessor {
    pub fn new(device_path: PathBuf, logger: Logger) -> Self {
        Self {
            device_path,
            config_path: PathBuf::from(MEASURED_CFG_DIR),
            logger,
        }
    }

    pub fn with_config_path(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config_path = dir.into();
        self
    }

    /// Writes configurations.
    async fn write_config_files(&self, initdata: &InitData) -> Result<()> {
        info!(
            self.logger,
            "Writing configuration files to: {:?}", self.config_path
        );

        if tokio::fs::try_exists(&self.config_path).await? {
            tokio::fs::remove_dir_all(&self.config_path).await?;
        }

        // Create the config_path.
        fs::create_dir_all(&self.config_path).context(format!(
            "Failed to create config path: {:?}",
            self.config_path
        ))?;

        // Set directory permissions (700).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&self.config_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.config_path, perms)?;
        }

        let mut written_files = 0;

        // Write each configuration item.
        for (key, value) in initdata.data() {
            let file_path = self.config_path.join(key).canonicalize()?;

            // Security check: Ensure file path is within the directory.
            if !file_path.starts_with(&self.config_path) {
                warn!(self.logger, "Skipping potentially dangerous key: {}", key);
                continue;
            }
            // TODO(burgerdev): support subdirectories

            self.write_file(&file_path, value.as_bytes())
                .await
                .context(format!("Failed to write config file for key: {}", key))?;

            written_files += 1;
        }

        info!(
            self.logger,
            "Successfully wrote {} configuration files", written_files
        );
        Ok(())
    }

    async fn write_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o444) // read-only for all users
            .open(path)
            .context(format!("Failed to open file: {:?}", path))?;

        file.write_all(content)
            .context(format!("Failed to write content to: {:?}", path))?;

        file.sync_all()
            .context(format!("Failed to sync file: {:?}", path))?;

        Ok(())
    }

    /// The complete workflow for processing initdata.
    pub async fn process(&self) -> Result<()> {
        info!(self.logger, "Starting initdata processing");

        // 1. Locate and parse initdata.
        info!(
            self.logger,
            "Reading initdata from device: {:?}", self.device_path
        );
        let initdata_content = read_initdata(&self.device_path)
            .await
            .context("Failed to read initdata: {e:?}")?;

        let initdata: InitData =
            toml::from_slice(&initdata_content).context("parse initdata failed")?;

        info!(
            self.logger,
            "Successfully parsed initdata with {} entries",
            initdata.data().len()
        );

        // TODO(burgerdev): 2. Validate initdata.

        // 3. Write config files.
        let mut initdata_path = self.config_path.clone();
        initdata_path.add_extension(".json");
        self.write_file(&initdata_path, &initdata_content).await?;
        self.write_config_files(&initdata).await?;

        info!(self.logger, "Initdata processing completed successfully");
        Ok(())
    }
}

fn create_logger() -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!())
}

#[tokio::main]
async fn main() -> Result<()> {
    let logger = create_logger();
    let initdata_device_opt = locate_device_concurrently(&logger).await?;
    let initdata_device = match initdata_device_opt {
        Some(device) => device,
        None => return Ok(()),
    };

    // Parse command line arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut processor = InitDataProcessor::new(initdata_device, logger);

    // Simple command line argument parsing.
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config-path" => {
                if i + 1 < args.len() {
                    processor = processor.with_config_path(&args[i + 1]);
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("--config-path requires a path argument"));
                }
            }
            _ => return Err(anyhow::anyhow!("Unknown argument: {}", args[i])),
        }
    }

    // Execute the processing.
    processor
        .process()
        .await
        .context("Initdata processing failed")?;

    Ok(())
}
