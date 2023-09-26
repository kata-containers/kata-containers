#!/usr/bin/env bats

. ./nvidia_init_functions 

cmdline="tsc=reliable no_timer_check rcupdate.rcu_expedited=1 i8042.direct=1 i8042.dumbkbd=1 i8042.nopnp=1 i8042.noaux=1 noreplace-smp reboot=k cryptomgr.notests net.ifnames=0 pci=lastbus=0 console=hvc0 console=hvc1 debug panic=1 nr_cpus=1 selinux=0 scsi_mod.scan=none agent.log=debug agent.debug_console agent.debug_console_vport=1026 agent.log=debug initcall_debug noccfilter nvidia.uvm.modprobe.options=\"uvm_enable_builtin_tests=1 uvm_perf_access_counter_mimc_migration_enable=1\" initrd=initrd"


@test "nvidia_process_kernel_params nvidia.uvm.modprobe.options" {
	nvidia_process_kernel_params "$cmdline" nvidia.uvm.modprobe.options
	echo "uvm_modprobe_options: $uvm_modprobe_options"
	[ "$uvm_modprobe_options" = "uvm_enable_builtin_tests=1 uvm_perf_access_counter_mimc_migration_enable=1" ]
}

setup() {
	nvidia-smi() {
		echo "nvidia-smi: test override"
	}
	nvidia_unload_reload_driver() {
		echo "nvidia_unload_reload_driver: test override"
	}
}

@test "nvidia_process_kernel_params nvidia_smi_lgc" {
	run nvidia_process_kernel_params "nvidia.smi.lgc=0:1200,1200"
	echo "bats status: $status"
	echo "bats output: $output"
	[ $status -eq 0 ]
	[ "${lines[1]}" = "nvidia: value of kernel param nvidia.smi.lgc: 0:1200,1200" ]
	[ "${lines[2]}" = "nvidia: locking gpu clocks on GPU0 to 1200,1200 MHz" ]
	[ "${lines[3]}" = "nvidia-smi: test override" ]
}


@test "nvidia_process_kernel_params nvidia_smi_lmcd" {
	run nvidia_process_kernel_params "nvidia.smi.lmcd=0:1500"
	echo "bats status: $status"
	echo "bats output: $output"
	[ $status -eq 0 ]
	[ "${lines[1]}" = "nvidia: value of kernel param nvidia.smi.lmcd: 0:1500" ]
	[ "${lines[2]}" = "nvidia: locking memory clocks on GPU0 to 1500 MHz" ]
	[ "${lines[3]}" = "nvidia-smi: test override" ]
	[ "${lines[4]}" = "nvidia_unload_reload_driver: test override" ]
}

@test "nvidia_process_kernel_params nvidia_smi_pl" {
	run nvidia_process_kernel_params "nvidia.smi.pl=0:250"
	echo "bats status: $status"
	echo "bats output: $output"
	[ $status -eq 0 ]
	[ "${lines[1]}" = "nvidia: value of kernel param nvidia.smi.pl: 0:250" ]
	[ "${lines[2]}" = "nvidia: setting power limit on GPU0 to 250 Watt" ]
	[ "${lines[3]}" = "nvidia-smi: test override" ]
}

@test "nvdiia_process_kernel_params nvidi_attestation_mode" {
	run nvidia_process_kernel_params "nvidia.attestation.mode=remote" nvidia.attestation.mode
	echo "bats status: $status"
	echo "bats output: $output"
	[ $status -eq 0 ]
	[ "${lines[1]}" = "nvidia: value of kernel param nvidia.attestation.mode: remote" ]
}