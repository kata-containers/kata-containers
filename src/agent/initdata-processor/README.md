# InitData Processor

## Overview

`initdata-processor` is a component of Kata Containers that processes initdata (initialization data) during virtual machine startup. It is responsible for:

1. Locating and identifying initdata devices
2. Reading and decompressing initdata content
3. Validating the structural integrity of initdata
4. Writing configuration files to the `/run/measured-cfg` directory

## Build and Installation

### Build Requirements

- Rust toolchain (1.92 or higher)
- Make

### Build Commands

```bash
# Default build (release mode)
make

# Or explicitly specify
make build

# Run tests
make test

# Code format check
make fmt

# Run clippy static analysis
make clippy

# Run all checks
make check

# Clean build artifacts
make clean
```

### Installation

```bash
# Install to default location (/usr)
sudo make install

# Specify installation prefix
sudo make install PREFIX=/usr/local

# Specify systemd directory
sudo make install SYSTEMD_DIR=/etc/systemd/system

# Uninstall
sudo make uninstall
```

Installed file locations:

- Binary: `/usr/bin/initdata-processor`
- Systemd service: `/usr/lib/systemd/system/initdata-processor.service`

## Usage

### Command Line Arguments

```bash
initdata-processor [OPTIONS]

OPTIONS:
    --config-path <PATH>    Configuration file output directory (default: /run/measured-cfg)
    --dev-path <PATH>       Device search directory (default: /dev)
```

### Examples

```bash
# Use default configuration
/usr/bin/initdata-processor

# Specify custom configuration path
/usr/bin/initdata-processor --config-path /custom/config/path

# Specify custom device path
/usr/bin/initdata-processor --dev-path /custom/dev/path
```

## Systemd Service Management

### Enable and Start Service

```bash
# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service (start on boot)
sudo systemctl enable initdata-processor.service

# Start service
sudo systemctl start initdata-processor.service

# Check service status
sudo systemctl status initdata-processor.service
```

### Service Configuration Details

The systemd service file is located at `/usr/lib/systemd/system/initdata-processor.service`:

```ini
[Unit]
Description=Kata InitData Processor
Documentation=https://github.com/kata-containers/kata-containers

[Service]
Type=oneshot
ExecStart=/usr/bin/initdata-processor
RemainAfterExit=yes
StandardOutput=journal
StandardError=journal
Restart=no

[Install]
WantedBy=multi-user.target
```

**Service Type Explanation:**

- `Type=oneshot`: Service executes once and exits, suitable for one-time initialization tasks
- `RemainAfterExit=yes`: Systemd considers the service active even after the process exits
- `Restart=no`: No automatic restart, as this is a one-time initialization task
