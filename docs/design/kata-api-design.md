# Kata API Design
To fulfill the [Kata design requirements](kata-design-requirements.md), and based on the discussion on [Virtcontainers API extensions](https://docs.google.com/presentation/d/1dbGrD1h9cpuqAPooiEgtiwWDGCYhVPdatq7owsKHDEQ), the Kata runtime library features the following APIs:
-  Sandbox based top API
-  Storage and network hotplug API
-  Plugin frameworks for external proprietary Kata runtime extensions
-  Built-in shim and proxy types and capabilities

## Sandbox Based API
### Sandbox Management API

|Name|Description|
|---|---|
|`CreateSandbox(SandboxConfig, Factory)`| Create a sandbox and its containers, base on `SandboxConfig` and `Factory`. Return the `Sandbox` structure, but do not start them.|

### Sandbox Operation API

|Name|Description|
|---|---|
|`sandbox.Delete()`| Shut down the VM in which the sandbox, and destroy the sandbox and remove all persistent metadata.|
|`sandbox.Monitor()`| Return a context handler for caller to monitor sandbox callbacks such as error termination.|
|`sandbox.Release()`| Release a sandbox data structure, close connections to the agent, and quit any goroutines associated with the Sandbox. Mostly used for daemon restart.|
|`sandbox.Start()`| Start a sandbox and the containers making the sandbox.|
|`sandbox.Stats()`| Get the stats of a running sandbox, return a `SandboxStats` structure.|
|`sandbox.Status()`| Get the status of the sandbox and containers, return a `SandboxStatus` structure.|
|`sandbox.Stop(force)`| Stop a sandbox and Destroy the containers in the sandbox. When force is true, ignore guest related stop failures.|
|`sandbox.CreateContainer(contConfig)`| Create new container in the sandbox with the `ContainerConfig` param. It will add new container config to `sandbox.config.Containers`.|
|`sandbox.DeleteContainer(containerID)`| Delete a container from the sandbox by containerID, return a `Container` structure.|
|`sandbox.EnterContainer(containerID, cmd)`| Run a new process in a container, executing customer's `types.Cmd` command.|
|`sandbox.KillContainer(containerID, signal, all)`| Signal a container in the sandbox by the containerID.|
|`sandbox.PauseContainer(containerID)`| Pause a running container in the sandbox by the containerID.|
|`sandbox.ProcessListContainer(containerID, options)`| List every process running inside a specific container in the sandbox, return a `ProcessList` structure.|
|`sandbox.ResumeContainer(containerID)`| Resume a paused container in the sandbox by the containerID.|
|`sandbox.StartContainer(containerID)`| Start a container in the sandbox by the containerID.|
|`sandbox.StatsContainer(containerID)`| Get the stats of a running container, return a `ContainerStats` structure.|
|`sandbox.StatusContainer(containerID)`| Get the status of a container in the sandbox, return a `ContainerStatus` structure.|
|`sandbox.StopContainer(containerID, force)`| Stop a container in the sandbox by the containerID.|
|`sandbox.UpdateContainer(containerID, resources)`| Update a running container in the sandbox.|
|`sandbox.WaitProcess(containerID, processID)`| Wait on a process to terminate.|
### Sandbox Hotplug API
|Name|Description|
|---|---|
|`sandbox.AddDevice(info)`| Add new storage device `DeviceInfo` to the sandbox, return a `Device` structure.|
|`sandbox.AddInterface(inf)`| Add new NIC to the sandbox.|
|`sandbox.RemoveInterface(inf)`| Remove a NIC from the sandbox.|
|`sandbox.ListInterfaces()`| List all NICs and their configurations in the sandbox, return a `pbTypes.Interface` list.|
|`sandbox.UpdateRoutes(routes)`| Update the sandbox route table (e.g. for portmapping support), return a `pbTypes.Route` list.|
|`sandbox.ListRoutes()`| List the sandbox route table, return a `pbTypes.Route` list.|

### Sandbox Relay API
|Name|Description|
|---|---|
|`sandbox.WinsizeProcess(containerID, processID, Height, Width)`| Relay TTY resize request to a process.|
|`sandbox.SignalProcess(containerID, processID, signalID, signalALL)`| Relay a signal to a process or all processes in a container.|
|`sandbox.IOStream(containerID, processID)`| Relay a process stdio. Return stdin/stdout/stderr pipes to the process stdin/stdout/stderr streams.|

### Sandbox Monitor API
|Name|Description|
|---|---|
|`sandbox.GetOOMEvent()`| Monitor the OOM events that occur in the sandbox..|
|`sandbox.UpdateRuntimeMetrics()`| Update the shim/hypervisor's metrics of the running sandbox.|
|`sandbox.GetAgentMetrics()`| Get metrics of the agent and the guest in the running sandbox.|

## Plugin framework for external proprietary Kata runtime extensions
### Hypervisor plugin

TBD.
### Metadata storage plugin
The metadata storage plugin controls where sandbox metadata is saved.
All metadata storage plugins must implement the following API:

|Name|Description|
|---|---|
|`storage.Save(key, value)`| Save a record.|
|`storage.Load(key)`| Load a record.|
|`storage.Delete(key)`| Delete a record.|

Built-in implementations include:
   -  Filesystem storage
   -  LevelDB storage

### VM Factory plugin
The VM factory plugin controls how a sandbox factory creates new VMs.
All VM factory plugins must implement following API:

|Name|Description|
|---|---|
|`VMFactory.NewVM(HypervisorConfig)`|Create a new VM based on `HypervisorConfig`.|

Built-in implementations include:

|Name|Description|
|---|---|
|`CreateNew()`| Create brand new VM based on `HypervisorConfig`.|
|`CreateFromTemplate()`| Create new VM from template.|
|`CreateFromCache()`| Create new VM from VM caches.|

### Sandbox Creation Plugin Workflow
![Sandbox Creation Plugin Workflow](https://raw.githubusercontent.com/bergwolf/raw-contents/master/kata/Kata-sandbox-creation.png "Sandbox Creation Plugin Workflow")

### Sandbox Connection Plugin Workflow
![Sandbox Connection Plugin Workflow](https://raw.githubusercontent.com/bergwolf/raw-contents/master/kata/Sandbox-Connection.png "Sandbox Connection Plugin Workflow")

## Built-in Shim and Proxy Types and Capabilities
### Built-in shim/proxy sandbox configurations
-  Supported shim configurations:

|Name|Description|
|---|---|
|`noopshim`|Do not start any shim process.|
|`ccshim`| Start the cc-shim binary.|
|`katashim`| Start the `kata-shim` binary.|
|`katashimbuiltin`|No standalone shim process but shim functionality APIs are exported.|
-  Supported proxy configurations:

|Name|Description|
|---|---|
|`noopProxy`| a dummy proxy implementation of the proxy interface, only used for testing purpose.|
|`noProxy`|generic implementation for any case where no actual proxy is needed.|
|`ccProxy`|run `ccProxy` to proxy between runtime and agent.|
|`kataProxy`|run `kata-proxy` to translate Yamux connections between runtime and Kata agent. |
|`kataProxyBuiltin`| no standalone proxy process and connect to Kata agent with internal Yamux translation.|

### Built-in Shim Capability
Built-in shim capability is implemented by removing standalone shim process, and
supporting the shim related APIs.

### Built-in Proxy Capability
Built-in proxy capability is achieved by removing standalone proxy process, and
connecting to Kata agent with a custom gRPC dialer that is internal Yamux translation.
The behavior is enabled when proxy is configured as `kataProxyBuiltin`.
