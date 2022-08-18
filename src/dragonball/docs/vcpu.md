# vCPU

## vCPU Manager
The vCPU manager is to manage all vCPU related actions, we will dive into some of the important structure members in this doc.

For now, aarch64 vCPU support is still under development, we'll introduce it when we merge `runtime-rs` to the master branch. (issue: #4445)

### vCPU config
`VcpuConfig` is used to configure guest overall CPU info.

`boot_vcpu_count` is used to define the initial vCPU number.

`max_vcpu_count` is used to define the maximum vCPU number and it's used for the upper boundary for CPU hotplug feature

`thread_per_core`, `cores_per_die`, `dies_per_socket` and `socket` are used to define CPU topology.

`vpmu_feature` is used to define `vPMU` feature level.
If `vPMU` feature is `Disabled`, it means `vPMU` feature is off (by default).
If `vPMU` feature is `LimitedlyEnabled`, it means minimal `vPMU` counters are supported (cycles and instructions).
If `vPMU` feature is `FullyEnabled`, it means all `vPMU` counters are supported

## vCPU State

There are four states for vCPU state machine: `running`, `paused`, `waiting_exit`, `exited`. There is a state machine to maintain the task flow.

When the vCPU is created, it'll turn to `paused` state. After vCPU resource is ready at VMM, it'll send a `Resume` event to the vCPU thread, and then vCPU state will change to `running`.

During the `running` state, VMM will catch vCPU exit and execute different logic according to the exit reason.

If the VMM catch some exit reasons that it cannot handle, the state will change to `waiting_exit` and VMM will stop the virtual machine. 
When the state switches to `waiting_exit`, an exit event will be sent to vCPU `exit_evt`, event manager will detect the change in `exit_evt` and set VMM `exit_evt_flag` as 1. A thread serving for VMM event loop will check `exit_evt_flag` and if the flag is 1, it'll stop the VMM.

When the VMM is stopped / destroyed, the state will change to `exited`.
   
## vCPU Hot plug
Since `Dragonball Sandbox` doesn't support virtualization of ACPI system, we use [`upcall`](https://github.com/openanolis/dragonball-sandbox/tree/main/crates/dbs-upcall) to establish a direct communication channel between `Dragonball` and Guest in order to trigger vCPU hotplug.

To use `upcall`, kernel patches are needed, you can get the patches from [`upcall`](https://github.com/openanolis/dragonball-sandbox/tree/main/crates/dbs-upcall) page, and we'll provide a ready-to-use guest kernel binary for you to try.

vCPU hot plug / hot unplug range is [1, `max_vcpu_count`]. Operations not in this range will be invalid.


