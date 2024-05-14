// Copyright (c) 2023 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::utils::{clear_cloexec, create_vhost_net_fds, open_named_tuntap};
use crate::{kernel_param::KernelParams, Address, HypervisorConfig};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::de;
use std::collections::HashMap;
use std::fmt::Display;
use std::fs::{read_to_string, File};
use std::os::fd::AsRawFd;
use tokio;

// These should have been called MiB and GiB for better readability but the
// more fitting names unfortunately generate linter warnings.
const MI_B: u64 = 1024 * 1024;
const GI_B: u64 = 1024 * MI_B;

// The approach taken here is inspired by govmm.  We build structs, each
// corresponding to a qemu command line parameter, like Kernel, or a device,
// for instance MemoryBackendFile.  Members of these structs mostly directly
// correspond to appropriate arguments of qemu parameters and are named
// the same except for '-' which are replaced with '_' in struct member names.
// The structs use a simple Builder pattern when necessary where mandatory
// arguments are passed to a constructor and setters are provided for the rest.
// All structs implement a simple ToQemuParams interface which allows their
// user to convert them to actual qemu command line parameter strings.

// There's nothing inherently async about this interface.  Unfortunately it
// has to be async anyway just due to the fact that QemuCmdLine holds a
// container of these, *and* due to the way QemuCmdLine is used -
// QemuInner::start_vm() happens to call an async function while a QemuCmdLine
// instance is on stack which makes it necessary for QemuCmdLine to be
// Send + Sync, and for that ToQemuParams has to be Send + Sync. :-(
#[async_trait]
trait ToQemuParams: Send + Sync {
    // OsString could look as a better fit here, however since foreign strings
    // come to this code from the outside as Strings already and this code adds
    // nothing but UTF-8 (in fact probably just ASCII) switching to OsStrings
    // now seems pointless.
    async fn qemu_params(&self) -> Result<Vec<String>>;
}

#[derive(Debug, PartialEq)]
enum VirtioBusType {
    Pci,
    Ccw,
}

impl VirtioBusType {
    fn as_str(&self) -> &str {
        match self {
            VirtioBusType::Pci => "pci",
            VirtioBusType::Ccw => "ccw",
        }
    }
}

impl Display for VirtioBusType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug)]
struct Kernel {
    // PathBuf would seem more appropriae but since we get the kernel path
    // from config as String already and we do no path operations on it,
    // converting to PathBuf and then back to String seems futile
    path: String,
    initrd_path: String,
    params: KernelParams,
}

impl Kernel {
    fn new(config: &HypervisorConfig) -> Result<Kernel> {
        // get kernel params
        let mut kernel_params = KernelParams::new(config.debug_info.enable_debug);

        if config.boot_info.initrd.is_empty() {
            // QemuConfig::validate() has already made sure that if initrd is
            // empty, image cannot be so we don't need to re-check that here

            kernel_params.append(
                &mut KernelParams::new_rootfs_kernel_params(
                    &config.boot_info.vm_rootfs_driver,
                    &config.boot_info.rootfs_type,
                )
                .context("adding rootfs params failed")?,
            );
        }

        kernel_params.append(&mut KernelParams::from_string(
            &config.boot_info.kernel_params,
        ));

        Ok(Kernel {
            path: config.boot_info.kernel.clone(),
            initrd_path: config.boot_info.initrd.clone(),
            params: kernel_params,
        })
    }
}

#[async_trait]
impl ToQemuParams for Kernel {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();

        // QemuConfig::adjust_config() ensures that kernel path is never empty
        result.push("-kernel".to_owned());
        result.push(self.path.clone());

        if !self.initrd_path.is_empty() {
            result.push("-initrd".to_owned());
            result.push(self.initrd_path.clone());
        }

        let kernel_params = self.params.to_string()?;
        if !kernel_params.is_empty() {
            result.push("-append".to_owned());
            result.push(kernel_params);
        }

        Ok(result)
    }
}

fn format_memory(mem_size: u64) -> String {
    if mem_size % GI_B == 0 {
        format!("{}G", mem_size / GI_B)
    } else if mem_size % MI_B == 0 {
        format!("{}M", mem_size / MI_B)
    } else {
        format!("{}", mem_size)
    }
}

#[derive(Debug)]
struct Memory {
    // 'size' and 'max_size' are stored in bytes
    size: u64,
    num_slots: u32,
    max_size: u64,
    memory_backend_file: Option<MemoryBackendFile>,
}

impl Memory {
    fn new(config: &HypervisorConfig) -> Memory {
        // Move this to QemuConfig::adjust_config()?

        let mut mem_size = config.memory_info.default_memory as u64;
        let mut max_mem_size = config.memory_info.default_maxmemory as u64;

        if let Ok(sysinfo) = nix::sys::sysinfo::sysinfo() {
            let host_memory = sysinfo.ram_total() >> 20;

            if mem_size > host_memory {
                info!(sl!(), "'default_memory' given in configuration.toml is greater than host memory, adjusting to host memory");
                mem_size = host_memory
            }

            if max_mem_size == 0 || max_mem_size > host_memory {
                max_mem_size = host_memory
            }
        } else {
            warn!(sl!(), "Failed to get host memory size, cannot verify or adjust configuration.toml's 'default_maxmemory'");

            if max_mem_size == 0 {
                max_mem_size = mem_size;
            };
        }

        // Memory sizes are given in megabytes in configuration.toml so we
        // need to convert them to bytes for storage.
        Memory {
            size: mem_size * MI_B,
            num_slots: config.memory_info.memory_slots,
            max_size: max_mem_size * MI_B,
            memory_backend_file: None,
        }
    }

    fn set_memory_backend_file(&mut self, mem_file: &MemoryBackendFile) -> &mut Self {
        if let Some(existing) = &self.memory_backend_file {
            if *existing != *mem_file {
                warn!(sl!(), "Memory: memory backend file already exists ({:?}) while trying to set a different one ({:?}), ignoring", existing, mem_file);
                return self;
            }
        }
        self.memory_backend_file = Some(mem_file.clone());
        self
    }
}

#[async_trait]
impl ToQemuParams for Memory {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();

        if self.size.trailing_zeros() < 19 {
            return Err(anyhow!(
                "bad memory size (must be given in whole megabytes): {}",
                self.size
            ));
        }
        params.push(format_memory(self.size));

        if self.num_slots != 0 {
            params.push(format!("slots={}", self.num_slots));
        }
        if self.max_size != 0 {
            params.push(format!("maxmem={}", format_memory(self.max_size)));
        }

        let mut retval = vec!["-m".to_owned(), params.join(",")];

        if let Some(mem_file) = &self.memory_backend_file {
            retval.append(&mut mem_file.qemu_params().await?);
        }
        Ok(retval)
    }
}

#[derive(Debug)]
struct Smp {
    num_vcpus: u32,
    max_num_vcpus: u32,
}

impl Smp {
    fn new(config: &HypervisorConfig) -> Smp {
        Smp {
            num_vcpus: config.cpu_info.default_vcpus as u32,
            max_num_vcpus: config.cpu_info.default_maxvcpus,
        }
    }
}

#[async_trait]
impl ToQemuParams for Smp {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        // CpuInfo::adjust_config() seems to ensure that both vcpu numbers
        // will have sanitised non-zero values
        params.push(format!("{}", self.num_vcpus));
        params.push(format!("maxcpus={}", self.max_num_vcpus));

        Ok(vec!["-smp".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct Cpu {
    cpu_features: String,
}

impl Cpu {
    fn new(config: &HypervisorConfig) -> Cpu {
        Cpu {
            cpu_features: config.cpu_info.cpu_features.clone(),
        }
    }
}

#[async_trait]
impl ToQemuParams for Cpu {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        // '-cpu host' has always to be used when using KVM
        let mut params = vec!["host".to_owned()];
        params.push(self.cpu_features.clone());
        Ok(vec!["-cpu".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct Machine {
    r#type: String,
    accel: String,
    options: String,
    nvdimm: bool,
    kernel_irqchip: Option<String>,

    is_nvdimm_supported: bool,
    memory_backend: Option<String>,
}

impl Machine {
    fn new(config: &HypervisorConfig) -> Machine {
        #[cfg(any(
            target_arch = "aarch64",
            target_arch = "powerpc64",
            target_arch = "x86",
            target_arch = "x86_64",
        ))]
        let is_nvdimm_supported = config.machine_info.machine_type != "microvm";
        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "powerpc64",
            target_arch = "x86",
            target_arch = "x86_64",
        )))]
        let is_nvdimm_supported = false;

        Machine {
            r#type: config.machine_info.machine_type.clone(),
            accel: "kvm".to_owned(),
            options: config.machine_info.machine_accelerators.clone(),
            nvdimm: false,
            kernel_irqchip: None,
            is_nvdimm_supported,
            memory_backend: None,
        }
    }

    fn set_nvdimm(&mut self, is_on: bool) -> &mut Self {
        if is_on && !self.is_nvdimm_supported {
            warn!(sl!(), "called to enable nvdimm but nvdimm is not supported");
        }
        self.nvdimm = is_on && self.is_nvdimm_supported;
        self
    }

    fn set_memory_backend(&mut self, mem_backend: &str) -> &mut Self {
        self.memory_backend = Some(mem_backend.to_owned());
        self
    }

    fn set_kernel_irqchip(&mut self, kernel_irqchip: &str) -> &mut Self {
        self.kernel_irqchip = Some(kernel_irqchip.to_owned());
        self
    }
}

#[async_trait]
impl ToQemuParams for Machine {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(self.r#type.clone());
        params.push(format!("accel={}", self.accel));
        if !self.options.is_empty() {
            params.push(self.options.clone());
        }
        if self.nvdimm {
            params.push("nvdimm=on".to_owned());
        }
        if let Some(kernel_irqchip) = &self.kernel_irqchip {
            params.push(format!("kernel_irqchip={}", kernel_irqchip));
        }
        if let Some(mem_backend) = &self.memory_backend {
            params.push(format!("memory-backend={}", mem_backend));
        }
        Ok(vec!["-machine".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct Knobs {
    no_user_config: bool,
    nodefaults: bool,
    nographic: bool,
    no_reboot: bool,
    no_shutdown: bool,
    daemonize: bool,
    stopped: bool,

    vga: String,
}

impl Knobs {
    fn new(_config: &HypervisorConfig) -> Knobs {
        Knobs {
            no_user_config: true,
            nodefaults: true,
            nographic: true,
            no_reboot: true,
            no_shutdown: false,
            daemonize: false,
            stopped: false,
            vga: "none".to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for Knobs {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();
        result.push("-vga".to_owned());
        result.push(self.vga.clone());
        if self.no_user_config {
            result.push("-no-user-config".to_owned());
        }
        if self.nodefaults {
            result.push("-nodefaults".to_owned());
        }
        if self.nographic {
            result.push("-nographic".to_owned());
        }
        if self.no_reboot {
            result.push("-no-reboot".to_owned());
        }
        if self.no_shutdown {
            result.push("-no-shutdown".to_owned());
        }
        if self.daemonize {
            result.push("-daemonize".to_owned());
        }
        if self.stopped {
            result.push("-S".to_owned());
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MemoryBackendFile {
    id: String,
    mem_path: String,
    size: u64,
    share: bool,
    readonly: bool,
}

impl MemoryBackendFile {
    fn new(id: &str, mem_path: &str, size: u64) -> MemoryBackendFile {
        MemoryBackendFile {
            id: id.to_string(),
            mem_path: mem_path.to_string(),
            size,
            share: false,
            readonly: false,
        }
    }

    fn set_share(&mut self, share: bool) -> &mut Self {
        self.share = share;
        self
    }

    fn set_readonly(&mut self, readonly: bool) -> &mut Self {
        self.readonly = readonly;
        self
    }
}

#[async_trait]
impl ToQemuParams for MemoryBackendFile {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("memory-backend-file".to_owned());
        params.push(format!("id={}", self.id));
        params.push(format!("mem-path={}", self.mem_path));
        params.push(format!("size={}", format_memory(self.size)));
        params.push(format!("share={}", if self.share { "on" } else { "off" }));
        params.push(format!(
            "readonly={}",
            if self.readonly { "on" } else { "off" }
        ));

        Ok(vec!["-object".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct TcpSocketOpts {
    host: String,
    // 'port' is required for a TCP socket
    port: String,
}

#[async_trait]
impl ToQemuParams for TcpSocketOpts {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        if !self.host.is_empty() {
            params.push(format!("host={}", self.host));
        }
        params.push(format!("port={}", self.port));
        Ok(params)
    }
}

#[derive(Debug)]
struct UnixSocketOpts {
    // 'path' is a required parameter for a unix socket
    path: String,
}

#[async_trait]
impl ToQemuParams for UnixSocketOpts {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("path={}", self.path));
        Ok(params)
    }
}

#[derive(Debug)]
enum ProtocolOptions {
    None,
    #[allow(dead_code)]
    Tcp(TcpSocketOpts),
    Unix(UnixSocketOpts),
}

#[async_trait]
impl ToQemuParams for ProtocolOptions {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let result = match self {
            ProtocolOptions::Tcp(tcp_opts) => tcp_opts.qemu_params().await?,
            ProtocolOptions::Unix(unix_opts) => unix_opts.qemu_params().await?,
            ProtocolOptions::None => Vec::new(),
        };
        Ok(result)
    }
}

#[derive(Debug)]
struct ChardevSocket {
    id: String,
    server: bool,
    wait: bool,
    protocol_options: ProtocolOptions,
}

impl ChardevSocket {
    fn new(id: &str) -> ChardevSocket {
        ChardevSocket {
            id: id.to_owned(),
            server: false,
            wait: true,
            protocol_options: ProtocolOptions::None,
        }
    }

    fn set_server(&mut self, server: bool) -> &mut Self {
        self.server = server;
        self
    }

    fn set_wait(&mut self, wait: bool) -> &mut Self {
        self.wait = wait;
        self
    }

    fn set_socket_opts(&mut self, opts: ProtocolOptions) -> &mut Self {
        self.protocol_options = opts;
        self
    }
}

#[async_trait]
impl ToQemuParams for ChardevSocket {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("socket".to_owned());
        params.push(format!("id={}", self.id));
        if self.server {
            params.push("server=on".to_owned());
            if self.wait {
                params.push("wait=on".to_owned());
            } else {
                params.push("wait=off".to_owned());
            }
        }
        params.append(&mut self.protocol_options.qemu_params().await?);
        Ok(vec!["-chardev".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct DeviceVhostUserFs {
    bus_type: VirtioBusType,
    chardev: String,
    tag: String,
    queue_size: u64,
    romfile: String,
    iommu_platform: bool,
}

impl DeviceVhostUserFs {
    fn new(chardev: &str, tag: &str, bus_type: VirtioBusType) -> DeviceVhostUserFs {
        DeviceVhostUserFs {
            bus_type,
            chardev: chardev.to_owned(),
            tag: tag.to_owned(),
            queue_size: 0,
            romfile: String::new(),
            iommu_platform: false,
        }
    }

    fn set_queue_size(&mut self, queue_size: u64) -> &mut Self {
        if queue_size <= 1024 && queue_size.is_power_of_two() {
            self.queue_size = queue_size;
        } else if queue_size != 0 {
            // zero is not an error here as it's treated as "value not set"
            // throughout runtime-rs
            warn!(
                sl!(),
                "bad vhost-user-fs-{} queue_size (must be power of two): {}, ignoring",
                self.bus_type,
                queue_size
            );
        }
        self
    }

    #[allow(dead_code)]
    fn set_romfile(&mut self, romfile: &str) -> &mut Self {
        self.romfile = romfile.to_owned();
        self
    }

    fn set_iommu_platform(&mut self, iommu_platform: bool) -> &mut Self {
        self.iommu_platform = iommu_platform;
        self
    }
}

#[async_trait]
impl ToQemuParams for DeviceVhostUserFs {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("vhost-user-fs-{}", self.bus_type));
        params.push(format!("chardev={}", self.chardev));
        params.push(format!("tag={}", self.tag));
        if self.queue_size != 0 {
            params.push(format!("queue-size={}", self.queue_size));
        }
        if !self.romfile.is_empty() {
            params.push(format!("romfile={}", self.romfile));
        }
        if self.iommu_platform {
            params.push("iommu_platform=on".to_owned());
        }
        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct DeviceNvdimm {
    memdev: String,
    unarmed: bool,
}

impl DeviceNvdimm {
    fn new(memdev: &str, unarmed: bool) -> DeviceNvdimm {
        DeviceNvdimm {
            memdev: memdev.to_owned(),
            unarmed,
        }
    }
}

#[async_trait]
impl ToQemuParams for DeviceNvdimm {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("nvdimm".to_owned());
        params.push(format!("memdev={}", self.memdev));
        if self.unarmed {
            params.push("unarmed=on".to_owned());
        }
        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct BlockBackend {
    driver: String,
    id: String,
    path: String,
    aio: String,
    cache_direct: bool,
    cache_no_flush: bool,
    read_only: bool,
}

impl BlockBackend {
    fn new(id: &str, path: &str) -> BlockBackend {
        BlockBackend {
            driver: "file".to_owned(),
            id: id.to_owned(),
            path: path.to_owned(),
            aio: "threads".to_owned(),
            cache_direct: true,
            cache_no_flush: false,
            read_only: true,
        }
    }

    #[allow(dead_code)]
    fn set_driver(&mut self, driver: &str) -> &mut Self {
        self.driver = driver.to_owned();
        self
    }

    #[allow(dead_code)]
    fn set_aio(&mut self, aio: &str) -> &mut Self {
        self.aio = aio.to_owned();
        self
    }

    #[allow(dead_code)]
    fn set_cache_direct(&mut self, cache_direct: bool) -> &mut Self {
        self.cache_direct = cache_direct;
        self
    }

    #[allow(dead_code)]
    fn set_cache_no_flush(&mut self, cache_no_flush: bool) -> &mut Self {
        self.cache_no_flush = cache_no_flush;
        self
    }

    #[allow(dead_code)]
    fn set_read_only(&mut self, read_only: bool) -> &mut Self {
        self.read_only = read_only;
        self
    }
}

#[async_trait]
impl ToQemuParams for BlockBackend {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("driver={}", self.driver));
        params.push(format!("node-name=image-{}", self.id));
        params.push(format!("filename={}", self.path));
        params.push(format!("aio={}", self.aio));
        if self.cache_direct {
            params.push("cache.direct=on".to_owned());
        } else {
            params.push("cache.direct=off".to_owned());
        }
        if self.cache_no_flush {
            params.push("cache.no-flush=on".to_owned());
        } else {
            params.push("cache.no-flush=off".to_owned());
        }
        if self.read_only {
            params.push("auto-read-only=on".to_owned());
        } else {
            params.push("auto-read-only=off".to_owned());
        }
        Ok(vec!["-blockdev".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct DeviceVirtioBlk {
    bus_type: VirtioBusType,
    id: String,
    scsi: bool,
    config_wce: bool,
    share_rw: bool,
}

impl DeviceVirtioBlk {
    fn new(id: &str, bus_type: VirtioBusType) -> DeviceVirtioBlk {
        DeviceVirtioBlk {
            bus_type,
            id: id.to_owned(),
            scsi: false,
            config_wce: false,
            share_rw: true,
        }
    }

    #[allow(dead_code)]
    fn set_scsi(&mut self, scsi: bool) -> &mut Self {
        self.scsi = scsi;
        self
    }

    #[allow(dead_code)]
    fn set_config_wce(&mut self, config_wce: bool) -> &mut Self {
        self.config_wce = config_wce;
        self
    }

    #[allow(dead_code)]
    fn set_share_rw(&mut self, share_rw: bool) -> &mut Self {
        self.share_rw = share_rw;
        self
    }
}

#[async_trait]
impl ToQemuParams for DeviceVirtioBlk {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("virtio-blk-{}", self.bus_type));
        params.push(format!("drive=image-{}", self.id));
        if self.scsi {
            params.push("scsi=on".to_owned());
        } else {
            params.push("scsi=off".to_owned());
        }
        if self.config_wce {
            params.push("config-wce=on".to_owned());
        } else {
            params.push("config-wce=off".to_owned());
        }
        if self.share_rw {
            params.push("share-rw=on".to_owned());
        } else {
            params.push("share-rw=off".to_owned());
        }
        params.push(format!("serial=image-{}", self.id));

        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

struct VhostVsock {
    bus_type: VirtioBusType,
    vhostfd: tokio::fs::File,
    guest_cid: u32,
    disable_modern: bool,
    iommu_platform: bool,
}

impl VhostVsock {
    fn new(vhostfd: tokio::fs::File, guest_cid: u32, bus_type: VirtioBusType) -> VhostVsock {
        VhostVsock {
            bus_type,
            vhostfd,
            guest_cid,
            disable_modern: false,
            iommu_platform: false,
        }
    }

    fn set_disable_modern(&mut self, disable_modern: bool) -> &mut Self {
        self.disable_modern = disable_modern;
        self
    }

    fn set_iommu_platform(&mut self, iommu_platform: bool) -> &mut Self {
        self.iommu_platform = iommu_platform;
        self
    }
}

#[async_trait]
impl ToQemuParams for VhostVsock {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("vhost-vsock-{}", self.bus_type));
        if self.disable_modern {
            params.push("disable-modern=true".to_owned());
        }
        if self.iommu_platform {
            params.push("iommu_platform=on".to_owned());
        }
        params.push(format!("vhostfd={}", self.vhostfd.as_raw_fd()));
        params.push(format!("guest-cid={}", self.guest_cid));

        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct NumaNode {
    memdev: String,
}

impl NumaNode {
    fn new(memdev: &str) -> NumaNode {
        NumaNode {
            memdev: memdev.to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for NumaNode {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("node".to_owned());
        params.push(format!("memdev={}", self.memdev));

        Ok(vec!["-numa".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct Serial {
    character_device: String,
}

impl Serial {
    #[allow(dead_code)]
    fn new(character_device: &str) -> Serial {
        Serial {
            character_device: character_device.to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for Serial {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        Ok(vec!["-serial".to_owned(), self.character_device.clone()])
    }
}

fn format_fds(files: &[File]) -> String {
    files
        .iter()
        .map(|file| file.as_raw_fd().to_string())
        .collect::<Vec<String>>()
        .join(":")
}

#[derive(Debug)]
struct Netdev {
    id: String,

    // File descriptors for vhost multi-queue support.
    // {
    //      queue_fds: Vec<File>,
    //      vhost_fds: Vec<File>,
    // }
    fds: HashMap<String, Vec<File>>,

    // disable_vhost_net disables virtio device emulation from the host kernel instead of from qemu.
    disable_vhost_net: bool,
}

impl Netdev {
    fn new(id: &str, host_if_name: &str, num_queues: u32) -> Result<Netdev> {
        let fds = HashMap::from([
            (
                "fds".to_owned(),
                open_named_tuntap(host_if_name, num_queues)?,
            ),
            ("vhostfds".to_owned(), create_vhost_net_fds(num_queues)?),
        ]);
        for file in fds.values().flatten() {
            clear_cloexec(file.as_raw_fd()).context("clearing O_CLOEXEC failed")?;
        }

        Ok(Netdev {
            id: id.to_owned(),
            fds,
            disable_vhost_net: false,
        })
    }

    fn set_disable_vhost_net(&mut self, disable_vhost_net: bool) -> &mut Self {
        self.disable_vhost_net = disable_vhost_net;
        self
    }
}

#[async_trait]
impl ToQemuParams for Netdev {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params: Vec<String> = Vec::new();
        params.push("tap".to_owned());
        params.push(format!("id={}", self.id));

        if !self.disable_vhost_net {
            params.push("vhost=on".to_owned());
            if let Some(vhost_fds) = self.fds.get("vhostfds") {
                params.push(format!("vhostfds={}", format_fds(vhost_fds)));
            }
        }

        if let Some(tuntap_fds) = self.fds.get("fds") {
            params.push(format!("fds={}", format_fds(tuntap_fds)));
        }

        Ok(vec!["-netdev".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
pub struct DeviceVirtioNet {
    // driver is the qemu device driver
    device_driver: String,

    // id is the corresponding backend net device identifier.
    netdev_id: String,

    // mac_address is the guest-side networking device interface MAC address.
    mac_address: Address,

    // disable_modern prevents qemu from relying on fast MMIO.
    disable_modern: bool,

    num_queues: u32,
    iommu_platform: bool,
}

impl DeviceVirtioNet {
    fn new(netdev_id: &str, mac_address: Address) -> DeviceVirtioNet {
        DeviceVirtioNet {
            device_driver: "virtio-net-pci".to_owned(),
            netdev_id: netdev_id.to_owned(),
            mac_address,
            disable_modern: false,
            num_queues: 1,
            iommu_platform: false,
        }
    }

    fn set_disable_modern(&mut self, disable_modern: bool) -> &mut Self {
        self.disable_modern = disable_modern;
        self
    }

    fn set_num_queues(&mut self, num_queues: u32) -> &mut Self {
        self.num_queues = num_queues;
        self
    }

    fn set_iommu_platform(&mut self, iommu_platform: bool) -> &mut Self {
        self.iommu_platform = iommu_platform;
        self
    }
}

#[async_trait]
impl ToQemuParams for DeviceVirtioNet {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params: Vec<String> = Vec::new();

        //params.push(format!("driver={}", &self.device_driver.to_string()));
        params.push(self.device_driver.clone());
        params.push(format!("netdev={}", &self.netdev_id));

        params.push(format!("mac={:?}", self.mac_address));

        if self.disable_modern {
            params.push("disable-modern=true".to_owned());
        }
        if self.iommu_platform {
            params.push("iommu_platform=on".to_owned());
        }

        params.push("mq=on".to_owned());
        params.push(format!("vectors={}", 2 * self.num_queues + 2));

        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct DeviceVirtioSerial {
    id: String,
    bus_type: VirtioBusType,
    iommu_platform: bool,
}

impl DeviceVirtioSerial {
    fn new(id: &str, bus_type: VirtioBusType) -> DeviceVirtioSerial {
        DeviceVirtioSerial {
            id: id.to_owned(),
            bus_type,
            iommu_platform: false,
        }
    }

    fn set_iommu_platform(&mut self, iommu_platform: bool) -> &mut Self {
        self.iommu_platform = iommu_platform;
        self
    }
}

#[async_trait]
impl ToQemuParams for DeviceVirtioSerial {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("virtio-serial-{}", self.bus_type));
        params.push(format!("id={}", self.id));
        if self.iommu_platform {
            params.push("iommu_platform=on".to_owned());
        }
        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

#[derive(Debug)]
struct DeviceVirtconsole {
    id: String,
    chardev: String,
}

impl DeviceVirtconsole {
    fn new(id: &str, chardev: &str) -> DeviceVirtconsole {
        DeviceVirtconsole {
            id: id.to_owned(),
            chardev: chardev.to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for DeviceVirtconsole {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("virtconsole".to_owned());
        params.push(format!("id={}", self.id));
        params.push(format!("chardev={}", self.chardev));
        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

// RTC represents a qemu Real Time Clock configuration.
#[derive(Debug)]
struct Rtc {
    // Base is the RTC start time.
    base: String,

    // Clock is the is the RTC clock driver.
    clock: String,

    // DriftFix is the drift fixing mechanism.
    driftfix: String,
}

impl Rtc {
    fn new() -> Rtc {
        Rtc {
            base: "utc".to_owned(),
            clock: "host".to_owned(),
            driftfix: "slew".to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for Rtc {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push(format!("base={}", self.base));
        params.push(format!("clock={}", self.clock));
        params.push(format!("driftfix={}", self.driftfix));
        Ok(vec!["-rtc".to_owned(), params.join(",")])
    }
}

// RngDevice represents a random number generator device.
#[derive(Debug)]
struct RngDevice {
    // id is the device ID
    id: String,

    // filename is the entropy source on the host
    filename: String,

    // max_bytes is the bytes allowed to guest to get from the host’s entropy per period
    max_bytes: u32,

    // period is duration of a read period in seconds
    period: u32,

    // transport is the virtio transport for this device.
    transport: String,
}

impl RngDevice {
    fn new() -> RngDevice {
        RngDevice {
            id: "rng0".to_owned(),
            filename: "/dev/urandom".to_owned(),
            max_bytes: 1024,
            period: 1000,
            transport: "virtio-rng-pci".to_owned(),
        }
    }
}

#[async_trait]
impl ToQemuParams for RngDevice {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut object_params = Vec::new();
        let mut device_params = Vec::new();

        object_params.push("rng-random".to_owned());
        object_params.push(format!("id={}", self.id));
        object_params.push(format!("filename={} ", self.filename));

        device_params.push(format!("-device {}", self.transport));
        device_params.push(format!("rng={}", self.id));
        device_params.push(format!("max_bytes={}", self.max_bytes));
        device_params.push(format!("period={}", self.period));

        Ok(vec![
            "-object".to_owned(),
            object_params.join(","),
            "-device".to_owned(),
            device_params.join(","),
        ])
    }
}

#[derive(Debug)]
struct DeviceIntelIommu {
    intremap: bool,
    device_iotlb: bool,
    caching_mode: bool,
}

impl DeviceIntelIommu {
    fn new() -> DeviceIntelIommu {
        DeviceIntelIommu {
            intremap: true,
            device_iotlb: true,
            caching_mode: true,
        }
    }
}

#[async_trait]
impl ToQemuParams for DeviceIntelIommu {
    async fn qemu_params(&self) -> Result<Vec<String>> {
        let mut params = Vec::new();
        params.push("intel-iommu".to_owned());
        let to_onoff = |b| if b { "on" } else { "off" };
        params.push(format!("intremap={}", to_onoff(self.intremap)));
        params.push(format!("device-iotlb={}", to_onoff(self.device_iotlb)));
        params.push(format!("caching-mode={}", to_onoff(self.caching_mode)));
        Ok(vec!["-device".to_owned(), params.join(",")])
    }
}

fn is_running_in_vm() -> Result<bool> {
    let res = read_to_string("/proc/cpuinfo")?
        .lines()
        .find(|line| line.starts_with("flags"))
        .ok_or(anyhow!("flags line not found"))?
        .split(' ')
        .map(String::from)
        .skip(1)
        .any(|flag| flag == "hypervisor");
    Ok(res)
}

fn should_disable_modern() -> bool {
    match is_running_in_vm() {
        Ok(retval) => retval,
        Err(err) => {
            info!(
                sl!(),
                "unable to check if running in VM, assuming not: {}", err
            );
            false
        }
    }
}

pub struct QemuCmdLine<'a> {
    id: String,
    config: &'a HypervisorConfig,

    kernel: Kernel,
    memory: Memory,
    smp: Smp,
    machine: Machine,
    cpu: Cpu,

    knobs: Knobs,

    devices: Vec<Box<dyn ToQemuParams>>,
}

impl<'a> QemuCmdLine<'a> {
    pub fn new(id: &str, config: &'a HypervisorConfig) -> Result<QemuCmdLine<'a>> {
        let mut qemu_cmd_line = QemuCmdLine {
            id: id.to_string(),
            config,
            kernel: Kernel::new(config)?,
            memory: Memory::new(config),
            smp: Smp::new(config),
            machine: Machine::new(config),
            cpu: Cpu::new(config),
            knobs: Knobs::new(config),
            devices: Vec::new(),
        };

        if config.device_info.enable_iommu {
            qemu_cmd_line.add_iommu();
        }

        qemu_cmd_line.add_rtc();

        qemu_cmd_line.add_rng();
      
        Ok(qemu_cmd_line)
    }

    fn add_rtc(&mut self) {
        let rtc = Rtc::new();
        self.devices.push(Box::new(rtc));
    }

    fn add_rng(&mut self) {
        let rng = RngDevice::new();
        self.devices.push(Box::new(rng));
    }

    fn bus_type(&self) -> VirtioBusType {
        if self.config.machine_info.machine_type.contains("-ccw-") {
            VirtioBusType::Ccw
        } else {
            VirtioBusType::Pci
        }
    }

    fn add_iommu(&mut self) {
        let dev_iommu = DeviceIntelIommu::new();
        self.devices.push(Box::new(dev_iommu));

        self.kernel
            .params
            .append(&mut KernelParams::from_string("intel_iommu=on iommu=pt"));

        self.machine.set_kernel_irqchip("split");
    }

    pub fn add_virtiofs_share(
        &mut self,
        virtiofsd_socket_path: &str,
        mount_tag: &str,
        queue_size: u64,
    ) {
        let chardev_name = "virtiofsd-chardev";

        // virtiofsd socket device
        let mut virtiofsd_socket_chardev = ChardevSocket::new(chardev_name);
        virtiofsd_socket_chardev.set_socket_opts(ProtocolOptions::Unix(UnixSocketOpts {
            path: virtiofsd_socket_path.to_owned(),
        }));

        self.devices.push(Box::new(virtiofsd_socket_chardev));

        let mut virtiofs_device = DeviceVhostUserFs::new(chardev_name, mount_tag, self.bus_type());
        virtiofs_device.set_queue_size(queue_size);
        if self.config.device_info.enable_iommu_platform && self.bus_type() == VirtioBusType::Ccw {
            virtiofs_device.set_iommu_platform(true);
        }
        self.devices.push(Box::new(virtiofs_device));

        let mut mem_file =
            MemoryBackendFile::new("entire-guest-memory-share", "/dev/shm", self.memory.size);
        mem_file.set_share(true);

        // don't put the /dev/shm memory backend file into the anonymous container,
        // there has to be at most one of those so keep it by name in Memory instead
        //self.devices.push(Box::new(mem_file));
        self.memory.set_memory_backend_file(&mem_file);

        match self.bus_type() {
            VirtioBusType::Pci => {
                self.machine.set_nvdimm(true);
                self.devices.push(Box::new(NumaNode::new(&mem_file.id)));
            }
            VirtioBusType::Ccw => {
                self.machine.set_memory_backend(&mem_file.id);
            }
        }
    }

    pub fn add_vsock(&mut self, vhostfd: tokio::fs::File, guest_cid: u32) -> Result<()> {
        clear_cloexec(vhostfd.as_raw_fd()).context("clearing O_CLOEXEC failed on vsock fd")?;

        let mut vhost_vsock_pci = VhostVsock::new(vhostfd, guest_cid, self.bus_type());

        if !self.config.disable_nesting_checks && should_disable_modern() {
            vhost_vsock_pci.set_disable_modern(true);
        }

        if self.config.device_info.enable_iommu_platform {
            vhost_vsock_pci.set_iommu_platform(true);
        }

        self.devices.push(Box::new(vhost_vsock_pci));
        Ok(())
    }

    pub fn add_nvdimm(&mut self, path: &str, is_readonly: bool) -> Result<()> {
        self.machine.set_nvdimm(true);
        if self.memory.max_size == 0 || self.memory.num_slots == 0 {
            info!(
                sl!(),
                "both memory max size and num slots must be set for nvdimm"
            );
            return Err(anyhow!(
                "both memory max size and num slots must be set for nvdimm"
            ));
        }

        let filesize = match std::fs::metadata(path) {
            Ok(metadata) => metadata.len(),
            Err(err) => {
                info!(sl!(), "couldn't get size of {}: {}", path, err);
                return Err(err.into());
            }
        };

        let mut mem_file = MemoryBackendFile::new("TODO", path, filesize);
        mem_file.set_readonly(is_readonly);
        self.devices.push(Box::new(mem_file));

        let nvdimm = DeviceNvdimm::new("TODO", is_readonly);
        self.devices.push(Box::new(nvdimm));

        Ok(())
    }

    pub fn add_block_device(&mut self, device_id: &str, path: &str) -> Result<()> {
        self.devices
            .push(Box::new(BlockBackend::new(device_id, path)));
        self.devices
            .push(Box::new(DeviceVirtioBlk::new(device_id, self.bus_type())));
        Ok(())
    }

    #[allow(dead_code)]
    pub fn add_serial_console(&mut self, character_device_file_path: &str) {
        let serial = Serial::new(character_device_file_path);
        self.devices.push(Box::new(serial));

        self.kernel.params.append(&mut KernelParams::from_string(
            "systemd.log_target=console console=ttyS0",
        ));
    }

    pub fn add_network_device(
        &mut self,
        dev_index: u64,
        host_dev_name: &str,
        guest_mac: Address,
    ) -> Result<()> {
        let mut netdev = Netdev::new(
            &format!("network-{}", dev_index),
            host_dev_name,
            self.config.network_info.network_queues,
        )?;
        if self.config.network_info.disable_vhost_net {
            netdev.set_disable_vhost_net(true);
        }

        let mut virtio_net_device = DeviceVirtioNet::new(&netdev.id, guest_mac);

        if should_disable_modern() {
            virtio_net_device.set_disable_modern(true);
        }
        if self.config.device_info.enable_iommu_platform && self.bus_type() == VirtioBusType::Ccw {
            virtio_net_device.set_iommu_platform(true);
        }
        if self.config.network_info.network_queues > 1 {
            virtio_net_device.set_num_queues(self.config.network_info.network_queues);
        }

        self.devices.push(Box::new(netdev));
        self.devices.push(Box::new(virtio_net_device));
        Ok(())
    }

    pub fn add_console(&mut self, console_socket_path: &str) {
        let mut serial_dev = DeviceVirtioSerial::new("serial0", self.bus_type());
        if self.config.device_info.enable_iommu_platform && self.bus_type() == VirtioBusType::Ccw {
            serial_dev.set_iommu_platform(true);
        }
        self.devices.push(Box::new(serial_dev));

        let chardev_name = "charconsole0";
        let console_device = DeviceVirtconsole::new("console0", chardev_name);
        self.devices.push(Box::new(console_device));

        let mut console_socket_chardev = ChardevSocket::new(chardev_name);
        console_socket_chardev.set_socket_opts(ProtocolOptions::Unix(UnixSocketOpts {
            path: console_socket_path.to_owned(),
        }));
        console_socket_chardev.set_server(true);
        console_socket_chardev.set_wait(false);
        self.devices.push(Box::new(console_socket_chardev));
    }

    pub async fn build(&self) -> Result<Vec<String>> {
        let mut result = Vec::new();

        result.append(&mut vec![
            "-name".to_owned(),
            format!("sandbox-{}", self.id),
        ]);
        result.append(&mut self.kernel.qemu_params().await?);
        result.append(&mut self.smp.qemu_params().await?);
        result.append(&mut self.machine.qemu_params().await?);
        result.append(&mut self.cpu.qemu_params().await?);
        result.append(&mut self.memory.qemu_params().await?);

        for device in &self.devices {
            result.append(&mut device.qemu_params().await?);
        }

        result.append(&mut self.knobs.qemu_params().await?);

        Ok(result)
    }
}
