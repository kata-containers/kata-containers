mod initdata;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use slog::{o, Drain, Logger};
use tracing::{error, info, warn, Level};
use tracing_subscriber::fmt::format::FmtSpan;

use crate::initdata::{locate_device_concurrently, read_initdata};
use kata_types::initdata::InitData;

const MEASURED_CFG_DIR: &str = "/run/measured-cfg";
const DEFAULT_VALIDATOR: &str = "/usr/bin/initdata-validator";

#[derive(Debug)]
struct InitDataProcessor {
    device_path: PathBuf,
    config_path: PathBuf,
    validator_path: PathBuf,
}

impl InitDataProcessor {
    pub fn new(device_path: &str) -> Self {
        Self {
            device_path: PathBuf::from(device_path),
            config_path: PathBuf::from(MEASURED_CFG_DIR),
            validator_path: PathBuf::from(DEFAULT_VALIDATOR),
        }
    }

    pub fn with_config_path(mut self, dir: impl Into<PathBuf>) -> Self {
        self.config_path = dir.into();
        self
    }

    pub fn with_validator(mut self, validator: impl Into<PathBuf>) -> Self {
        self.validator_path = validator.into();
        self
    }

    /// Reads and parses initdata from the device.
    async fn parse_initdata(&self) -> Result<InitData> {
        info!("Reading initdata from device: {:?}", self.device_path);
        let initdata_content = read_initdata(&self.device_path)
            .await
            .map_err(|e| anyhow!("Failed to read initdata: {e:?}"))?;

        let initdata: InitData =
            toml::from_slice(&initdata_content).context("parse initdata failed")?;

        info!(
            "Successfully parsed initdata with {} entries",
            initdata.data().len()
        );

        Ok(initdata)
    }

    /// Validates initdata using an external binary.
    async fn validate_initdata(&self, initdata: &InitData) -> Result<()> {
        info!("Validating initdata using: {:?}", self.validator_path);

        if !self.validator_path.exists() {
            warn!("validator not found at {:?}", self.validator_path);
            initdata.validate()?;
            return Ok(());
        }

        let mut child = Command::new(&self.validator_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("Failed to spawn validator: {:?}", self.validator_path))?;

        // Send initdata to the validator.
        if let Some(stdin) = child.stdin.as_mut() {
            let serialized = serde_json::to_string(initdata)
                .context("Failed to serialize initdata for validation")?;
            stdin
                .write_all(serialized.as_bytes())
                .context("Failed to write initdata to validator stdin")?;
        }

        let output = child
            .wait_with_output()
            .context("Failed to wait for validator completion")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Initdata validation failed: exit code {:?}, stderr: {}",
                output.status.code(),
                stderr
            ));
        }

        info!("Initdata validation successful");
        Ok(())
    }

    /// Writes configurations.
    async fn write_config_files(&self, initdata: &InitData) -> Result<()> {
        info!("Writing configuration files to: {:?}", self.config_path);

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
            perms.set_mode(0o700);
            fs::set_permissions(&self.config_path, perms)?;
        }

        let mut written_files = 0;

        // Write each configuration item.
        for (key, value) in initdata.data() {
            let file_path = self.config_path.join(key);

            // Security check: Ensure file path is within the directory.
            if !file_path.starts_with(&self.config_path) {
                warn!("Skipping potentially dangerous key: {}", key);
                continue;
            }

            self.write_secure_file(&file_path, value.as_bytes())
                .await
                .context(format!("Failed to write config file for key: {}", key))?;

            written_files += 1;
        }

        info!("Successfully wrote {} configuration files", written_files);
        Ok(())
    }

    /// Securely writes a file.
    async fn write_secure_file(&self, path: &Path, content: &[u8]) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o600) // Read/write only for owner.
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
        info!("Starting initdata processing");

        // 1. Locate and parse initdata.
        let initdata = self.parse_initdata().await?;

        // 2. Validate initdata.
        self.validate_initdata(&initdata).await?;

        // 3. Write config files.
        self.write_config_files(&initdata).await?;

        info!("Initdata processing completed successfully");
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
    std::panic::set_hook(Box::new(|panic_info| {
        error!(panic.info = %panic_info, "A task panicked");
    }));

    // Initialize the tracing subscriber to configure the logging format.
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(true)
        .init();

    let logger = create_logger();
    let initdata_device_opt = locate_device_concurrently(&logger).await?;
    let initdata_device = match initdata_device_opt {
        Some(device) => device,
        None => return Ok(()),
    };

    // Parse command line arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut processor = InitDataProcessor::new(&initdata_device);

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
            "--validator" => {
                if i + 1 < args.len() {
                    processor = processor.with_validator(&args[i + 1]);
                    i += 2;
                } else {
                    return Err(anyhow::anyhow!("--validator requires a path argument"));
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
