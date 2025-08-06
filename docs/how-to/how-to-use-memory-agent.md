# How to use mem-agent to decrease the memory usage of Kata container
## Introduction
mem-agent is a component designed for managing memory in Linux environments.<br>
The mem-agent has been integrated into the kata-agent to reduce memory usage in Kata containers.

## Open mem-agent in configuration
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/configuration.toml"
$ sudo sed -i -e 's/^#mem_agent_enable.*$/mem_agent_enable = true/g' $config_file
```

## Open reclaim_guest_freed_memory in configuration
Enabling this will result in the VM balloon device having f_reporting=on set.<br>
Then the hypervisor will use it to reclaim guest freed memory.

When mem-agent reclaim the memory of the guest, this function will reclaim guest freed memory in the host.

**To use mem-agent, must open reclaim_guest_freed_memory in configuration.**

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/configuration.toml"
$ sudo sed -i -e 's/^#reclaim_guest_freed_memory.*$/reclaim_guest_freed_memory = true/g' $config_file
```

## Sub-feature psi
During memory reclamation and compaction, mem-agent monitors system pressure using Pressure Stall Information (PSI).<br>
If the system pressure becomes too high, memory reclamation or compaction will automatically stop.

This feature helps the mem-agent reduce its overhead on system performance.

## Sub-feature memcg
Use the Linux kernel MgLRU feature to monitor each cgroup's memory usage and periodically reclaim cold memory.

During each run period, memcg calls the run_aging function of MgLRU for each cgroup to mark the hot and cold states of the pages within it.<br>
Then, it calls the run_eviction function of MgLRU for each cgroup to reclaim a portion of the cold pages that have not been accessed for three periods.

After the run period, the memcg will enter a sleep period. Once the sleep period is over, it will transition into the next run period, and this cycle will continue.

**The following are the configurations of the sub-feature memcg:**

### memcg_disable
Control the mem-agent memcg function disable or enable.<br>
Default to false.
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_disable.*$/memcg_disable = true/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"disabled":true}'
```

### memcg_swap
If this feature is disabled, the mem-agent will only track and reclaim file cache pages.  If this feature is enabled, the mem-agent will handle both file cache pages and anonymous pages.<br>
Default to false.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_swap.*$/memcg_swap = true/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"swap":true}'
```

#### setup guest swap
memcg_swap should use with guest swap function.<br>
The guest swap function will create a separate swap task that will create and insert swap files into the guest as needed.<br>
Just dragonball and cloud-hypervisor support guest swap.

Use following configuration to enable guest swap.
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#enable_guest_swap.*$/enable_guest_swap = true/g' $config_file
```

By default, swap files are created in the /run/kata-containers/swap directory. You can use the following configuration to create swap files in a different directory.
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#guest_swap_path.*$/guest_swap_path = \"\/run\/kata-containers\/swap\"/g' $config_file
```

By default, the inserted swap file will match the current memory size, which is set to 100%. You can modify the percentage of the swap size relative to the current memory size using the configuration below.
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#guest_swap_size_percent.*$/guest_swap_size_percent = 100/g' $config_file
```

The swap task will wait for 60 seconds before determining the memory size and creating swap files. This approach helps prevent interference with the startup performance of the kata container during its initial creation and avoids frequent insertion of swap files when the guest memory size is adjusted frequently. You can configure the waiting time using the option below.
```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#guest_swap_create_threshold_secs.*$/guest_swap_create_threshold_secs = 60/g' $config_file
```

### memcg_swappiness_max
The usage of this value is similar to the swappiness in the Linux kernel, applying a ratio of swappiness_max/200 when utilized.<br>
At the beginning of the eviction memory process for a cgroup in each run period, the coldest anonymous pages are assigned a maximum eviction value based on swappiness_max/200.<br>
When the run_eviction function of MgLRU is actually called, if the comparison ratio between the current coldest anonymous pages and file cache pages exceeds this value, then this value will be used as the swappiness.<br>
Default to 50.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_swappiness_max.*$/memcg_swappiness_max = 50/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"swappiness_max":50}'
```

### memcg_period_secs
Control the mem-agent memcg function wait period seconds.<br>
Default to 600.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_period_secs.*$/memcg_period_secs = 600/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"period_secs":600}'
```

### memcg_period_psi_percent_limit
Control the mem-agent memcg wait period PSI percent limit.<br>
If the percentage of memory and IO PSI stall time within the memcg waiting period for a cgroup exceeds this value, then the memcg run period for this cgroup will not be executed after this waiting period.<br>
Default to 1

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_period_psi_percent_limit.*$/memcg_period_psi_percent_limit = 1/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"period_psi_percent_limit":1}'
```

### memcg_eviction_psi_percent_limit
Control the mem-agent memcg eviction PSI percent limit.<br>
If the percentage of memory and IO PSI stall time for a cgroup exceeds this value during an eviction cycle, the eviction for this cgroup will immediately stop and will not resume until the next memcg waiting period.<br>
Default to 1.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_eviction_psi_percent_limit.*$/memcg_eviction_psi_percent_limit = 1/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"eviction_psi_percent_limit":1}'
```

### memcg_eviction_run_aging_count_min
Control the mem-agent memcg eviction run aging count min.<br>
A cgroup will only perform eviction when the number of aging cycles in memcg is greater than or equal to memcg_eviction_run_aging_count_min.<br>
Default to 3.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#memcg_eviction_run_aging_count_min.*$/memcg_eviction_run_aging_count_min = 3/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentMemcgSet json://{"eviction_run_aging_count_min":3}'
```

## Sub-feature compact
The memory control group (memcg) functionality may release a significant number of small pages, but the VM balloon free page reporting feature used by reclaim_guest_freed_memory requires at least a contiguous block of order 10 pages(a page block) to be released from the host.<br>
The sub-feature compact is designed to address the issue of fragmented pages.<br>

During each run period, compact check the continuity of free pages within the system. If necessary, the compact will invoke the Linux compaction feature to reorganize fragmented pages.<br>
After the run period, the compact will enter a sleep period. Once the sleep period is over, it will transition into the next run period, and this cycle will continue.

*the VM balloon free page reporting feature in arm64_64k report order 5 pages. Following is the comments from Linux kernel.*
```
		/*
		 * The default page reporting order is @pageblock_order, which
		 * corresponds to 512MB in size on ARM64 when 64KB base page
		 * size is used. The page reporting won't be triggered if the
		 * freeing page can't come up with a free area like that huge.
		 * So we specify the page reporting order to 5, corresponding
		 * to 2MB. It helps to avoid THP splitting if 4KB base page
		 * size is used by host.
		 *
		 * Ideally, the page reporting order is selected based on the
		 * host's base page size. However, it needs more work to report
		 * that value. The hard-coded order would be fine currently.
		 */
```

**The following are the configurations of the sub-feature compact:**

### compact_disable
Control the mem-agent compact function disable or enable.<br>
Default to false.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_disable.*$/compact_disable = true/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"disabled":false}'
```

### compact_period_secs
Control the mem-agent compaction function wait period seconds.<br>
Default to 600.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_period_secs.*$/compact_period_secs = 600/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"period_secs":600}'
```

### compact_period_psi_percent_limit
Control the mem-agent compaction function wait period PSI percent limit.<br>
If the percentage of memory and IO PSI stall time within the compaction waiting period exceeds this value, then the compaction will not be executed after this waiting period.<br>
Default to 1.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_period_psi_percent_limit.*$/compact_period_psi_percent_limit = 1/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"period_psi_percent_limit":1}'
```

### compact_psi_percent_limit
Control the mem-agent compaction function compact PSI percent limit.<br>
During compaction, the percentage of memory and IO PSI stall time is checked every second. If this percentage exceeds compact_psi_percent_limit, the compaction process will stop.<br>
Default to 5

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_psi_percent_limit.*$/compact_psi_percent_limit = 5/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"compact_psi_percent_limit":5}'
```

### compact_sec_max
Control the maximum number of seconds for each compaction of mem-agent compact function.<br>
If compaction seconds is bigger than compact_sec_max during compact run period, stop compaction at once.

Default to 180.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_sec_max.*$/compact_sec_max = 180/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"compact_sec_max":180}'
```

### compact_order
compact_order is use with compact_threshold.<br>
compact_order parameter determines the size of contiguous pages that the mem-agent's compaction functionality aims to achieve.<br>
For example, if compact_order is set to 10 in a Kata container guest environment, the compaction function will target acquiring more contiguous pages of order 10, which will allow reclaim_guest_freed_memory to release additional pages.<br>
If the goal is to have more free pages of order 9 in the system to ensure a higher likelihood of obtaining transparent huge pages during memory allocation, then setting compact_order to 9 would be appropriate.
Default to 9.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_order.*$/compact_order = 9/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"compact_order":9}'
```

### compact_threshold
Control the mem-agent compaction function compact threshold.<br>
compact_threshold is the pages number.<br>
When examining the /proc/pagetypeinfo, if there's an increase in the number of movable pages of orders smaller than the compact_order compared to the amount following the previous compaction period, and this increase surpasses a certain threshold specifically, more than compact_threshold number of pages, or the number of free pages has decreased by compact_threshold since the previous compaction. Current compact run period will not do compaction because there is no enough fragmented pages to be compaction.<br>
This design aims to minimize the impact of unnecessary compaction calls on system performance.<br>
Default to 1024.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_threshold.*$/compact_threshold = 1024/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"compact_threshold":1024}'
```

### compact_force_times
Control the mem-agent compaction function force compact times.<br>
After one compaction during a run period, if there are consecutive instances of compact_force_times run periods where no compaction occurs, a compaction will be forced regardless of the system's memory state.<br>
If compact_force_times is set to 0, will do force compaction each period.<br>
If compact_force_times is set to 18446744073709551615, will never do force compaction.<br>
Default to 18446744073709551615.

```bash
$ config_file="/opt/kata/share/defaults/kata-containers/runtime-rs/configuration.toml"
$ sudo sed -i -e 's/^#compact_force_times.*$/compact_force_times = 18446744073709551615/g' $config_file
```

For a running Kata container, this configuration can be dynamically modified using the kata-agent-ctl command.
```bash
$ PODID="12345"
$ kata-agent-ctl connect --server-address "unix:///var/run/kata/$PODID/root/kata.hvsock" --hybrid-vsock \
--cmd 'MemAgentCompactSet json://{"compact_force_times":18446744073709551615}'
```
