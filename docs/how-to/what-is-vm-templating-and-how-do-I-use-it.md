# What Is VM Templating and How To Enable It

### What is VM templating
VM templating is a Kata Containers feature that enables new VM
creation using a cloning technique. When enabled, new VMs are created
by cloning from a pre-created template VM, and they will share the
same initramfs, kernel and agent memory in readonly mode. It is very
much like a process fork done by the kernel but here we *fork* VMs.

### How is this different from VMCache
Both [VMCache](../how-to/what-is-vm-cache-and-how-do-I-use-it.md) and VM templating help speed up new container creation.  
When VMCache enabled, new VMs are created by the VMCache server.  So it is not vulnerable to share memory CVE because each VM doesn't share the memory.  
VM templating saves a lot of memory if there are many Kata Containers running on the same host.

### What are the Pros
VM templating helps speed up new container creation and saves a lot
of memory if there are many Kata Containers running on the same host.
If you are running a density workload, or care a lot about container
startup speed, VM templating can be very useful.

In one example, we created 100 Kata Containers each claiming 128MB
guest memory and ended up saving 9GB of memory in total when VM templating
is enabled, which is about 72% of the total guest memory. See [full results
here](https://github.com/kata-containers/runtime/pull/303#issuecomment-395846767).

In another example, we created ten Kata Containers with containerd shimv2
and calculated the average boot up speed for each of them. The result
showed that VM templating speeds up Kata Containers creation by as much as
38.68%. See [full results here](https://gist.github.com/bergwolf/06974a3c5981494a40e2c408681c085d).

### What are the Cons
One drawback of VM templating is that it cannot avoid cross-VM side-channel
attack such as [CVE-2015-2877](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2015-2877)
that originally targeted at the Linux KSM feature.
It was concluded that "Share-until-written approaches for memory conservation among
mutually untrusting tenants are inherently detectable for information disclosure,
and can be classified as potentially misunderstood behaviors rather than vulnerabilities."

**Warning**: If you care about such attack vector, do not use VM templating or KSM.

### How to enable VM templating
VM templating can be enabled by changing your Kata Containers config file (`/usr/share/defaults/kata-containers/configuration.toml`,
overridden by `/etc/kata-containers/configuration.toml` if provided) such that:

  - `qemu-lite` is specified in `hypervisor.qemu`->`path` section
  - `enable_template = true`
  - `initrd =` is set
  - `image =` option is commented out or removed
  - `shared_fs` should not be `virtio-fs`

Then you can create a VM templating for later usage by calling
```
$ sudo kata-runtime factory init
```
and purge it by calling
```
$ sudo kata-runtime factory destroy
```

If you do not want to call `kata-runtime factory init` by hand,
the very first Kata container you create will automatically create a VM templating.
