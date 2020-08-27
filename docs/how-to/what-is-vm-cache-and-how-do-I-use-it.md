# What Is VMCache and How To Enable It

* [What is VMCache](#what-is-vmcache)
* [How is this different to VM templating](#how-is-this-different-to-vm-templating)
* [How to enable VMCache](#how-to-enable-vmcache)
* [Limitations](#limitations)

### What is VMCache

VMCache is a new function that creates VMs as caches before using it.
It helps speed up new container creation.  
The function consists of a server and some clients communicating
through Unix socket.  The protocol is gRPC in [`protocols/cache/cache.proto`](../../src/runtime/protocols/cache/cache.proto).  
The VMCache server will create some VMs and cache them by factory cache.
It will convert the VM to gRPC format and transport it when gets
requested from clients.  
Factory `grpccache` is the VMCache client.  It will request gRPC format
VM and convert it back to a VM.  If VMCache function is enabled,
`kata-runtime` will request VM from factory `grpccache` when it creates
a new sandbox.

### How is this different to VM templating

Both [VM templating](../how-to/what-is-vm-templating-and-how-do-I-use-it.md) and VMCache help speed up new container creation.  
When VM templating enabled, new VMs are created by cloning from a pre-created template VM, and they will share the same initramfs, kernel and agent memory in readonly mode.  So it saves a lot of memory if there are many Kata Containers running on the same host.  
VMCache is not vulnerable to [share memory CVE](../how-to/what-is-vm-templating-and-how-do-I-use-it.md#what-are-the-cons) because each VM doesn't share the memory.

### How to enable VMCache

VMCache can be enabled by changing your Kata Containers config file (`/usr/share/defaults/kata-containers/configuration.toml`,
overridden by `/etc/kata-containers/configuration.toml` if provided) such that:
* `vm_cache_number` specifies the number of caches of VMCache:
    *  unspecified or == 0  
       VMCache is disabled
    * `> 0`  
      will be set to the specified number
*  `vm_cache_endpoint` specifies the address of the Unix socket.

Then you can create a VM templating for later usage by calling:
```
$ sudo kata-runtime factory init
```
and purge it by `ctrl-c` it.

### Limitations
* Cannot work with VM templating.
* Only supports the QEMU hypervisor.
