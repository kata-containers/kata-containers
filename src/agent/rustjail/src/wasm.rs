// Copyright (c) 2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

macro_rules! cfg_arch_support_wasm {
    ($($item:item)*) => {
        $(
            #[cfg(any(
                target_arch = "x86_64",
                target_arch = "x86",
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "riscv64",
                target_arch = "riscv32"
            ))]
            $item
        )*
    }
}

macro_rules! cfg_arch_not_support_wasm {
    ($($item:item)*) => {
        $(
            #[cfg(not(any(
                target_arch = "x86_64",
                target_arch = "x86",
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "riscv64",
                target_arch = "riscv32"
            )))]
            $item
        )*
    }
}

use std::process::exit;

use crate::container::CLOG_FD;
use crate::log_child;
use crate::sync::write_count;

cfg_arch_support_wasm! {
    use anyhow::Result;
    use std::os::unix::io::RawFd;

    #[cfg(feature = "standard-oci-runtime")]
    use crate::console;

    use wasmer::{Instance, Module, Store};
    use wasmer_compiler_cranelift::Cranelift;
    use wasmer_engine_universal::Universal;
    use wasmer_wasi::WasiState;

    #[cfg(feature = "standard-oci-runtime")]
    use crate::container::CONSOLE_SOCKET_FD;
    use crate::container::{
        do_child_setup, do_child_setup_release, exec_env_build, CRFD_FD, CWFD_FD, FIFO_FD,
        INIT, NO_PIVOT,
    };
    use crate::sync::{write_sync, SYNC_FAILED};

    pub fn run_wasm() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let instance = do_setup_wrapper();
        let start = instance.exports.get_function("_start")?;
        start.call(&[])?;

        Ok(())
    }

    fn do_setup_wrapper() -> Instance {
        let cwfd = std::env::var(CWFD_FD).unwrap().parse::<i32>().unwrap();
        let cfd_log = std::env::var(CLOG_FD).unwrap().parse::<i32>().unwrap();

        match do_setup(cwfd, cfd_log) {
            Ok(instance) => {
                log_child!(cfd_log, "wasm do_setup successfully");
                instance
            }
            Err(e) => {
                log_child!(cfd_log, "wasm do_setup error {:?}", e);
                let _ = write_sync(cwfd, SYNC_FAILED, format!("{:?}", e).as_str());
                exit(0);
            }
        }
    }

    fn do_setup(cwfd: RawFd, cfd_log: RawFd) -> Result<Instance> {
        let init = std::env::var(INIT)?.eq(format!("{}", true).as_str());
        let no_pivot = std::env::var(NO_PIVOT)?.eq(format!("{}", true).as_str());
        let crfd = std::env::var(CRFD_FD)?.parse::<i32>()?;

        let mut fifofd = -1;
        if init {
            fifofd = std::env::var(FIFO_FD)?.parse::<i32>()?;
        }

        let exec_env = exec_env_build(init, no_pivot, crfd, cwfd, cfd_log, fifofd);

        #[cfg(feature = "standard-oci-runtime")]
        let csocket_fd = console::setup_console_socket(&std::env::var(CONSOLE_SOCKET_FD)?)?;
        #[cfg(feature = "seccomp")]
        let (args, oci_process, linux) = do_child_setup(&exec_env)?;
        #[cfg(not(feature = "seccomp"))]
        let (args, oci_process, _) = do_child_setup(&exec_env)?;

        let instance = do_setup_wasm(args[0].clone(), &args[1..])?;

        log_child!(cfd_log, "ready to run wasm");

        do_child_setup_release(
            &exec_env,
            oci_process,
            #[cfg(feature = "seccomp")]
            linux,
            #[cfg(feature = "standard-oci-runtime")]
            csocket_fd,
        )?;

        Ok(instance)
    }

    fn do_setup_wasm(wasm_path: String, wargs: &[String]) -> Result<Instance> {
        let compiler_config = Cranelift::default();
        let engine = Universal::new(compiler_config).engine();
        let store = Store::new(&engine);

        let mut wasi_env = WasiState::new(wasm_path.clone()).args(wargs).finalize()?;

        let module = Module::from_file(&store, wasm_path)?;
        let import_object = wasi_env.import_object(&module)?;
        let instance = Instance::new(&module, &import_object)?;

        let _ = instance.exports.get_function("_start")?;

        Ok(instance)
    }

    pub fn arch_support_wasm() -> bool {
        true
    }
}

cfg_arch_not_support_wasm! {
    pub fn run_wasm() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfd_log = std::env::var(CLOG_FD).unwrap().parse::<i32>().unwrap();
        log_child!(cfd_log, "wasm is not support by current architecture");
        exit(0);
    }

    pub fn arch_support_wasm() -> bool {
        false
    }
}
