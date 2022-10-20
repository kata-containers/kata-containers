# Multi-vmm support for runtime-rs
Some key points for supporting multi-vmm in rust runtime.

## 1. Hypervisor Config

The diagram below gives an overview for the hypervisor config

![hypervisor config](../../docs/images/hypervisor-config.svg)

VMM's config info will be loaded when initialize the runtime instance, there are some important functions need to be focused on. 
### `VirtContainer::init()`

This function initialize the runtime handler. It will register the plugins into the HYPERVISOR_PLUGINS. Different plugins are needed for different hypervisors. 
```rust
#[async_trait]
impl RuntimeHandler for VirtContainer {
    fn init() -> Result<()> {
        // register
        let dragonball_config = Arc::new(DragonballConfig::new());
        register_hypervisor_plugin("dragonball", dragonball_config);
        Ok(())
    }
}
```

[This is the plugin method for QEMU. Other VMM plugin methods haven't support currently.](../../../libs/kata-types/src/config/hypervisor/qemu.rs)
QEMU plugin defines the methods to adjust and validate the hypervisor config file, those methods could be modified if it is needed.

After that, when loading the TOML config, the plugins will be called to adjust and validate the config file.
```rust
async fn try_init(&mut self, spec: &oci::Spec) -> Result<()> {、
    ...
    let config = load_config(spec).context("load config")?;
    ...
}
```

### new_instance

This function will create a runtime_instance which include the operations for container and sandbox.  At the same time, a hypervisor instance will be created.  QEMU instance will be created here as well, and set the hypervisor config file
```rust
async fn new_hypervisor(toml_config: &TomlConfig) -> Result<Arc<dyn Hypervisor>> {
    let hypervisor_name = &toml_config.runtime.hypervisor_name;
    let hypervisor_config = toml_config
        .hypervisor
        .get(hypervisor_name)
        .ok_or_else(|| anyhow!("failed to get hypervisor for {}", &hypervisor_name))
        .context("get hypervisor")?;

    // TODO: support other hypervisor
    match hypervisor_name.as_str() {
        HYPERVISOR_DRAGONBALL => {
            let mut hypervisor = Dragonball::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        _ => Err(anyhow!("Unsupported hypervisor {}", &hypervisor_name)),
    }
}
```

## 2. Hypervisor Trait

[To support multi-vmm, the hypervisor trait need to be implemented.](./src/lib.rs)
```rust
pub trait Hypervisor: Send + Sync {
    // vm manager
    async fn prepare_vm(&self, id: &str, netns: Option<String>) -> Result<()>;
    async fn start_vm(&self, timeout: i32) -> Result<()>;
    async fn stop_vm(&self) -> Result<()>;
    async fn pause_vm(&self) -> Result<()>;
    async fn save_vm(&self) -> Result<()>;
    async fn resume_vm(&self) -> Result<()>;
    
    // device manager
    async fn add_device(&self, device: device::Device) -> Result<()>;
    async fn remove_device(&self, device: device::Device) -> Result<()>;
    
    // utils
    async fn get_agent_socket(&self) -> Result<String>;
    async fn disconnect(&self);
    async fn hypervisor_config(&self) -> HypervisorConfig;
    async fn get_thread_ids(&self) -> Result<VcpuThreadIds>;
    async fn get_pids(&self) -> Result<Vec<u32>>;
    async fn cleanup(&self) -> Result<()>;
    async fn check(&self) -> Result<()>;
    async fn get_jailer_root(&self) -> Result<String>;
    async fn save_state(&self) -> Result<HypervisorState>;
   }
```
### VM start procedure: steps through time
In current design, VM will be started in the following steps.

![vmm start](../../docs/images/vm-start.svg)

### Semantic of each function of the trait
In runtime-rs, most functions' implementation are VMM-specific, since currently `Dragonball` is default
VMM and implemented correctly, following section will explain two things.
1. In high-level, what is the expected behavior of each function.
2. What `DragonBall Sandbox`(DBS) does in each function. This could be a reference, but remember that each
VMM should have their own implementations.

#### Explanations
```rust
async fn prepare_vm(&self, id: &str, netns: Option<String>) -> Result<()>
```
1. What is expected?
* In general, `prepare_vm()` need the VMM to prepare everything **that VMM needs before boot**. 
For example, maybe fill in some fields of VMM, or maybe create something on host that the VMM needs when booting. But only some preparations need to be done.


2. What DBS does?
* DBS fills in some of its fields, e.g. `id, state, jailer_root, vm_path, netns`, and create a Unix Domain Socket(uds) for communication. Note that the uds is not directly added to the vmm, but added to a pending list. The devices in the pending list will be added after the VM starts.

---

```rust
async fn start_vm(&self, timeout: i32) -> Result<()>
```
1. What is expected?
* In general, `start_vm()` need the VMM and kernel to start and function correctly, which means ALL other functions from the trait `Hypervisor` should be able to return an expected result. This may be based on the preparation that `prepare_vm()` has done.

2. What DBS does?
* DBS first run its VMM server.
    * It creates the jailer path of the VM, setup the channel for thread communication, and start a thread to run its event_loop, since DBS runs as the thread within the process. Other VMM may start another process.
* DBS then starts its kernel.
    * It does normal things as a VMM, prepare kernel_params, initrd, image, rootfs, etc. and runs with its kernel started.

---

```rust
async fn stop_vm(&self) -> Result<()>
```
1. What is expected?
* In general, `stop_vm()` wants you to terminate the VMM from its execution.

2. What DBS does?
* It sends an stopVM request to DBS VMM server, the VMM server will receive the request and exit its execution from the thread called `vmm_thread`.

---

```rust
    async fn pause_vm(&self) -> Result<()>;
```
1. What is expected?
* In general, `pause_vm()` wants you to pause the VMM from its execution like it said. The ways to actually pause the VMM differs from VMMs.

2. What DBS does?
* Not implemented in DBS.

---

```rust
    async fn resume_vm(&self) -> Result<()>;
```
1. What is expected?
* This is a paired function with `pause_vm()`, it resumes the VMM's execution from the paused state.

2. What DBS does?
* Not implemented in DBS.

---

```rust
    async fn save_vm(&self) -> Result<()>;
```
1. What is expected?
* TBD

2. What DBS does?
* Not implemented in DBS.

---
    
```rust
async fn add_device(&self, device: device::Device) -> Result<()>;
```
1. What is expected?
* This functions is a generic interface of adding devices which implement trait `device::Device`. If the VMM
is started, the device should be hotplugged. If the VMM have not started, the device should be in some pending list (s.t. it is never forgotten) and plugged when VM starts.

2. What DBS does?
* If DBS has booted, it hotplugs the device if supported. Else, it puts the devices in a pending list and plugs them when DBS boots.
* Currently, devices include `Network device`, `VFIO device`, `Block device`, `Vsock`, `ShareFsDevice`, `ShareFsMount`.
    * DBS treats `ShareFsDevice and ShareFsMount` as devices, but this should be VMM-specific.

---

```rust
    async fn remove_device(&self, device: device::Device) -> Result<()>;
```
1. What is expected?
* In general, `remove_device()` asks the VMM to remove the device which implemented trait `device::Device`.

2. What DBS does?
* DBS currently only supports remove block drive, other devices are not supported to be removed.

---
    
```rust
    async fn get_agent_socket(&self) -> Result<String>;
```
1. What is expected?
* In Kata Containers, runtime talks to agent through a VSock. In the agent, the agent use VMADDR_CID_ANY to bind to the vsock.
> Note: Therefore, the VSock has to be inserted before the agent start. Note that the agent is booted by an init system on guest kernel. If systemd is supported, it uses systemd; If not, set AGENT_INIT environment variable, and guest kernel will use kata agent as /sbin/init

2. What DBS does?
* DBS simply returns the path of the vsock on the host side. Note that DBS add the vsock in the pending list in `prepare_vm()`
and insert this `cold_start_vm()`.

---

```rust
    async fn disconnect(&self);
```
1. What is expected?
* This currently is not called anywhere in runtime-rs, but semantically means the VMM should not be answering any request from runtime.

2. What DBS does?
* DBS sets its state to `VmmState::NotReady`.

---

```rust
    async fn hypervisor_config(&self) -> HypervisorConfig;
```
1. What is expected?
* Returns the hypervisor's config like it said, the configuration is deserialized from toml configuration file.

2. What DBS does?
* Same above.

---

```rust
async fn get_thread_ids(&self) -> Result<VcpuThreadIds>;
```
1. What is expected?
* Returns the thread id of vcpus. This is for contraining the vcpu thread by cgroup.

2. What DBS does?
* Same above.

---

```rust
async fn get_pids(&self) -> Result<Vec<u32>>;
```
1. What is expected?
* Returns the thread ids related to the VMM, **EXCEPT FOR** the vcpus' thread ids.

2. What DBS does?
* Same above.

---

```rust
async fn cleanup(&self) -> Result<()>;
```
1. What is expected?
* Cleanup the resources used by VMM, e.g. kernel, rootfs.

2. What DBS does?
* Same above.

---

```rust
async fn check(&self) -> Result<()>;
```
1. What is expected?
* TBD.

2. What DBS does?
* DBS returns Ok(()).

---

```rust
async fn get_jailer_root(&self) -> Result<String>;
```
1. What is expected?
* Returns the jailer root path on host side as it said.

2. What DBS does?
* Same above.

---

```rust
async fn save_state(&self) -> Result<HypervisorState>;
```
1. What is expected?
* This is for the persistance, a.k.a trait `Persist`. Fill in the `HypervisorState` struct and returns.

2. What DBS does?
* Same above.

#### Some suggestions
1. VSock insertion is a must for agent to boot.
2. Key functions are `prepare_vm()` and `start_vm()`, these are the most important ones. Most of the other functions can easily be done after this.
