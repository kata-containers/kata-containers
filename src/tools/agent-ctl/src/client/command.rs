use crate::{
    client::{DEFAULT_PROC_SIGNAL, ERR_API_FAILED},
    types::Options,
    utils,
};
use anyhow::{anyhow, Result};
use byteorder::ByteOrder;
use protocols::{
    agent::*, agent_ttrpc::AgentServiceClient, health::CheckRequest, health_ttrpc::HealthClient,
};
use slog::{debug, info};
use std::{thread::sleep, time::Duration};
use ttrpc::context::Context;

const REQUEST_BUILD_FAIL_MESSAGE: &str = "Fail to build request";

// Run the specified closure to set an automatic value if the ttRPC Context
// does not contain the special values requesting automatic values be
// suppressed.
macro_rules! check_auto_values {
    ($ctx:expr, $closure:expr) => {{
        let cfg = $ctx.metadata.get(super::METADATA_CFG_NS);

        if let Some(v) = cfg {
            if v.contains(&super::NO_AUTO_VALUES_CFG_NAME.to_string()) {
                debug!(sl!(), "Running closure to generate values");

                if let Err(e) = $closure() {
                    return (Err(e), false);
                }
            }
        }
    }};
}

pub fn parse_agent_cmd(cmd: &str) -> Result<Box<dyn AgentCmd>> {
    match cmd {
        "AddARPNeighbors" => Ok(Box::new(AddARPNeighbors {})),

        "AddSwap" => Ok(Box::new(AddSwap {})),

        "Check" => Ok(Box::new(Check {})),

        "Version" => Ok(Box::new(Version {})),

        "CloseStdin" => Ok(Box::new(CloseStdin {})),

        "CopyFile" => Ok(Box::new(CopyFile {})),

        "CreateContainer" => Ok(Box::new(CreateContainer {})),

        "CreateSandbox" => Ok(Box::new(CreateSandbox {})),

        "DestroySandbox" => Ok(Box::new(DestroySandbox {})),

        "ExecProcess" => Ok(Box::new(ExecProcess {})),

        "GetGuestDetails" => Ok(Box::new(GetGuestDetails {})),

        "GetIptables" => Ok(Box::new(GetIptables {})),

        "GetMetrics" => Ok(Box::new(GetMetrics {})),

        "GetOOMEvent" => Ok(Box::new(GetOOMEvent {})),

        "GetVolumeStats" => Ok(Box::new(GetVolumeStats {})),

        "ListInterfaces" => Ok(Box::new(ListInterfaces {})),

        "ListRoutes" => Ok(Box::new(ListRoutes {})),

        "MemHotplugByProbe" => Ok(Box::new(MemHotplugByProbe {})),

        "OnlineCPUMem" => Ok(Box::new(OnlineCPUMem {})),

        "PauseContainer" => Ok(Box::new(PauseContainer {})),

        "ReadStderr" => Ok(Box::new(ReadStderr {})),

        "ReadStdout" => Ok(Box::new(ReadStdout {})),

        "ReseedRandomDev" => Ok(Box::new(ReseedRandomDev {})),

        "RemoveContainer" => Ok(Box::new(RemoveContainer {})),

        "ResumeContainer" => Ok(Box::new(ResumeContainer {})),

        "SetGuestDateTime" => Ok(Box::new(SetGuestDateTime {})),

        "SetIptables" => Ok(Box::new(SetIptables {})),

        "SignalProcess" => Ok(Box::new(SignalProcess {})),

        "StartContainer" => Ok(Box::new(StartContainer {})),

        "StatsContainer" => Ok(Box::new(StatsContainer {})),

        "TtyWinResize" => Ok(Box::new(TtyWinResize {})),

        "UpdateContainer" => Ok(Box::new(UpdateContainer {})),

        "UpdateInterface" => Ok(Box::new(UpdateInterface {})),

        "UpdateRoutes" => Ok(Box::new(UpdateRoutes {})),

        "WaitProcess" => Ok(Box::new(WaitProcess {})),

        "WriteStdin" => Ok(Box::new(WriteStdin {})),

        _ => Err(anyhow!("Invalid command: {:?}", cmd)),
    }
}

pub trait AgentCmd {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool);
}

struct AddARPNeighbors;

impl AgentCmd for AddARPNeighbors {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: AddARPNeighborsRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        // FIXME: Implement fully.
        eprintln!("FIXME: 'AddARPNeighbors' not fully implemented");

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.add_arp_neighbors(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct AddSwap;

impl AgentCmd for AddSwap {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        _args: &str,
    ) -> (Result<()>, bool) {
        let req = AddSwapRequest::default();

        // FIXME: Implement 'AddSwap' fully.
        eprintln!("FIXME: 'AddSwap' not fully implemented");

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.add_swap(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct Check;

impl AgentCmd for Check {
    fn exec(
        &self,
        ctx: &Context,
        _client: &AgentServiceClient,
        health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: CheckRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match health.check(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct Version;

impl AgentCmd for Version {
    fn exec(
        &self,
        ctx: &Context,
        _client: &AgentServiceClient,
        health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        // XXX: Yes, the API is actually broken!
        let req: CheckRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match health.version(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct CloseStdin;

impl AgentCmd for CloseStdin {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: CloseStdinRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.close_stdin(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct CopyFile;

impl AgentCmd for CopyFile {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: CopyFileRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let path = utils::get_option("path", options, args)?;
            if !path.is_empty() {
                req.set_path(path);
            }

            let file_size_str = utils::get_option("file_size", options, args)?;

            if !file_size_str.is_empty() {
                let file_size = file_size_str
                    .parse::<i64>()
                    .map_err(|e| anyhow!(e).context("invalid file_size"))?;

                req.set_file_size(file_size);
            }

            let file_mode_str = utils::get_option("file_mode", options, args)?;

            if !file_mode_str.is_empty() {
                let file_mode = file_mode_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid file_mode"))?;

                req.set_file_mode(file_mode);
            }

            let dir_mode_str = utils::get_option("dir_mode", options, args)?;

            if !dir_mode_str.is_empty() {
                let dir_mode = dir_mode_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid dir_mode"))?;

                req.set_dir_mode(dir_mode);
            }

            let uid_str = utils::get_option("uid", options, args)?;

            if !uid_str.is_empty() {
                let uid = uid_str
                    .parse::<i32>()
                    .map_err(|e| anyhow!(e).context("invalid uid"))?;

                req.set_uid(uid);
            }

            let gid_str = utils::get_option("gid", options, args)?;

            if !gid_str.is_empty() {
                let gid = gid_str
                    .parse::<i32>()
                    .map_err(|e| anyhow!(e).context("invalid gid"))?;
                req.set_gid(gid);
            }

            let offset_str = utils::get_option("offset", options, args)?;

            if !offset_str.is_empty() {
                let offset = offset_str
                    .parse::<i64>()
                    .map_err(|e| anyhow!(e).context("invalid offset"))?;
                req.set_offset(offset);
            }

            let data_str = utils::get_option("data", options, args)?;
            if !data_str.is_empty() {
                let data = utils::str_to_bytes(&data_str)?;
                req.set_data(data.to_vec());
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.copy_file(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct CreateContainer;

impl AgentCmd for CreateContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: CreateContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        // FIXME: container create: add back "spec=file:///" support

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;
            let ttrpc_spec = utils::get_ttrpc_spec(options, &cid).map_err(|e| anyhow!(e))?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);
            req.set_OCI(ttrpc_spec);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.create_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct CreateSandbox;

impl AgentCmd for CreateSandbox {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: CreateSandboxRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let sid = utils::get_option("sid", options, args)?;
            req.set_sandbox_id(sid);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.create_sandbox(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct DestroySandbox;

impl AgentCmd for DestroySandbox {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: DestroySandboxRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.destroy_sandbox(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), true)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), true),
        }
    }
}

struct ExecProcess;

impl AgentCmd for ExecProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: ExecProcessRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            let ttrpc_spec = utils::get_ttrpc_spec(options, &cid).map_err(|e| anyhow!(e))?;

            let bundle_dir = options
                .get("bundle-dir")
                .ok_or("BUG: bundle-dir missing")
                .map_err(|e| anyhow!(e))?;

            let process = ttrpc_spec
                .Process
                .into_option()
                .ok_or(format!(
                    "failed to get process from OCI spec: {}",
                    bundle_dir,
                ))
                .map_err(|e| anyhow!(e))?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);
            req.set_process(process);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.exec_process(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct GetGuestDetails;

impl AgentCmd for GetGuestDetails {
    #[allow(clippy::redundant_closure_call)]
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: GuestDetailsRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            req.set_mem_block_size(true);
            req.set_mem_hotplug_probe(true);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.get_guest_details(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct GetIptables;

impl AgentCmd for GetIptables {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: GetIPTablesRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.get_ip_tables(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct GetMetrics;

impl AgentCmd for GetMetrics {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: GetMetricsRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.get_metrics(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct GetOOMEvent;

impl AgentCmd for GetOOMEvent {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: GetOOMEventRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.get_oom_event(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct GetVolumeStats;

impl AgentCmd for GetVolumeStats {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: VolumeStatsRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.get_volume_stats(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ListInterfaces;

impl AgentCmd for ListInterfaces {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: ListInterfacesRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.list_interfaces(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ListRoutes;

impl AgentCmd for ListRoutes {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: ListRoutesRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.list_routes(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct MemHotplugByProbe;

impl AgentCmd for MemHotplugByProbe {
    #[allow(clippy::redundant_closure_call)]
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: MemHotplugByProbeRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        // Expected to be a comma separated list of hex addresses
        let addr_list = match utils::get_option("memHotplugProbeAddr", options, args) {
            Ok(val) => val,
            Err(e) => return (Err(e), false),
        };

        check_auto_values!(ctx, || -> Result<()> {
            if !addr_list.is_empty() {
                let addrs: Vec<u64> = addr_list
                    // Convert into a list of string values.
                    .split(',')
                    // Convert each string element into a u8 array of bytes, ignoring
                    // those elements that fail the conversion.
                    .filter_map(|s| hex::decode(s.trim_start_matches("0x")).ok())
                    // "Stretch" the u8 byte slice into one of length 8
                    // (to allow each 8 byte chunk to be converted into a u64).
                    .map(|mut v| -> Vec<u8> {
                        v.resize(8, 0x0);
                        v
                    })
                    // Convert the slice of u8 bytes into a u64
                    .map(|b| byteorder::LittleEndian::read_u64(&b))
                    .collect();

                req.set_memHotplugProbeAddr(addrs);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.mem_hotplug_by_probe(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct OnlineCPUMem;

impl AgentCmd for OnlineCPUMem {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: OnlineCPUMemRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let wait_str = utils::get_option("wait", options, args)?;

            if !wait_str.is_empty() {
                let wait = wait_str
                    .parse::<bool>()
                    .map_err(|e| anyhow!(e).context("invalid wait bool"))?;

                req.set_wait(wait);
            }

            let nb_cpus_str = utils::get_option("nb_cpus", options, args)?;

            if !nb_cpus_str.is_empty() {
                let nb_cpus = nb_cpus_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid nb_cpus value"))?;

                req.set_nb_cpus(nb_cpus);
            }

            let cpu_only_str = utils::get_option("cpu_only", options, args)?;

            if !cpu_only_str.is_empty() {
                let cpu_only = cpu_only_str
                    .parse::<bool>()
                    .map_err(|e| anyhow!(e).context("invalid cpu_only bool"))?;

                req.set_cpu_only(cpu_only);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.online_cpu_mem(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct PauseContainer;

impl AgentCmd for PauseContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: PauseContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;

            req.set_container_id(cid);
            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.pause_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ReadStderr;

impl AgentCmd for ReadStderr {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: ReadStreamRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);

            let length_str = utils::get_option("len", options, args)?;

            if !length_str.is_empty() {
                let length = length_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid length"))?;
                req.set_len(length);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.read_stderr(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ReadStdout;

impl AgentCmd for ReadStdout {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: ReadStreamRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);

            let length_str = utils::get_option("len", options, args)?;

            if !length_str.is_empty() {
                let length = length_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid length"))?;
                req.set_len(length);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.read_stdout(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ReseedRandomDev;

impl AgentCmd for ReseedRandomDev {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: ReseedRandomDevRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let str_data = utils::get_option("data", options, args)?;
            let data = utils::str_to_bytes(&str_data)?;

            req.set_data(data.to_vec());

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.reseed_random_dev(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct RemoveContainer;

impl AgentCmd for RemoveContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: RemoveContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            req.set_container_id(cid);
            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.remove_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct ResumeContainer;

impl AgentCmd for ResumeContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: ResumeContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;

            req.set_container_id(cid);
            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.resume_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct SetGuestDateTime;

impl AgentCmd for SetGuestDateTime {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: SetGuestDateTimeRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let secs_str = utils::get_option("sec", options, args)?;

            if !secs_str.is_empty() {
                let secs = secs_str
                    .parse::<i64>()
                    .map_err(|e| anyhow!(e).context("invalid seconds"))?;

                req.set_Sec(secs);
            }

            let usecs_str = utils::get_option("usec", options, args)?;

            if !usecs_str.is_empty() {
                let usecs = usecs_str
                    .parse::<i64>()
                    .map_err(|e| anyhow!(e).context("invalid useconds"))?;

                req.set_Usec(usecs);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.set_guest_date_time(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct SetIptables;

impl AgentCmd for SetIptables {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: SetIPTablesRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.set_ip_tables(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct SignalProcess;

impl AgentCmd for SignalProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: SignalProcessRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            let mut sigstr = utils::get_option("signal", options, args)?;

            // Convert to a numeric
            if sigstr.is_empty() {
                sigstr = DEFAULT_PROC_SIGNAL.to_string();
            }

            let signum = utils::signame_to_signum(&sigstr).map_err(|e| anyhow!(e))?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);
            req.set_signal(signum as u32);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.signal_process(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct StartContainer;

impl AgentCmd for StartContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: StartContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;

            req.set_container_id(cid);
            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.start_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct StatsContainer;

impl AgentCmd for StatsContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: StatsContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;

            req.set_container_id(cid);
            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.stats_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct TtyWinResize;

impl AgentCmd for TtyWinResize {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: TtyWinResizeRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);

            let rows_str = utils::get_option("row", options, args)?;

            if !rows_str.is_empty() {
                let rows = rows_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid row size"))?;
                req.set_row(rows);
            }

            let cols_str = utils::get_option("column", options, args)?;

            if !cols_str.is_empty() {
                let cols = cols_str
                    .parse::<u32>()
                    .map_err(|e| anyhow!(e).context("invalid column size"))?;

                req.set_column(cols);
            }

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.tty_win_resize(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct UpdateContainer;

impl AgentCmd for UpdateContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: UpdateContainerRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;

            req.set_container_id(cid);

            Ok(())
        });

        // FIXME: Implement fully
        eprintln!("FIXME: 'UpdateContainer' not fully implemented");

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.update_container(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct UpdateInterface;

impl AgentCmd for UpdateInterface {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: UpdateInterfaceRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        // FIXME: Implement 'UpdateInterface' fully.
        eprintln!("FIXME: 'UpdateInterface' not fully implemented");

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.update_interface(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct UpdateRoutes;

impl AgentCmd for UpdateRoutes {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        _options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let req: UpdateRoutesRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        // FIXME: Implement 'UpdateRoutes' fully.
        eprintln!("FIXME: 'UpdateRoutes' not fully implemented");

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.update_routes(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct WaitProcess;

impl AgentCmd for WaitProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: WaitProcessRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.wait_process(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

struct WriteStdin;

impl AgentCmd for WriteStdin {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        _health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        let mut req: WriteStreamRequest = match utils::make_request(args) {
            Ok(res) => res,
            Err(e) => {
                return (
                    Err(anyhow!("{:?}", e).context(REQUEST_BUILD_FAIL_MESSAGE)),
                    false,
                )
            }
        };

        check_auto_values!(ctx, || -> Result<()> {
            let cid = utils::get_option("cid", options, args)?;
            let exec_id = utils::get_option("exec_id", options, args)?;

            let str_data = utils::get_option("data", options, args)?;
            let data = utils::str_to_bytes(&str_data)?;

            req.set_container_id(cid);
            req.set_exec_id(exec_id);
            req.set_data(data.to_vec());

            Ok(())
        });

        debug!(sl!(), "sending request"; "request" => format!("{:?}", req));

        match client.write_stdin(ctx.clone(), &req) {
            Ok(res) => {
                info!(sl!(), "response received"; "response" => format!("{:?}", res));
                (Ok(()), false)
            }
            Err(e) => (Err(anyhow!("{:?}", e).context(ERR_API_FAILED)), false),
        }
    }
}

pub fn parse_builtin_cmd(cmd: &str) -> Result<Box<dyn BuiltinCmd>> {
    match cmd {
        "help" => Ok(Box::new(Help {})),

        "echo" => Ok(Box::new(Echo {})),

        "list" => Ok(Box::new(List {})),

        "repeat" => Ok(Box::new(Repeat {})),

        "sleep" => Ok(Box::new(Sleep {})),

        "quit" => Ok(Box::new(Quit {})),

        _ => Err(anyhow!("Invalid command: {:?}", cmd)),
    }
}

pub trait BuiltinCmd {
    fn exec(&self, args: &str) -> (Result<()>, bool);
}

struct Echo;

impl BuiltinCmd for Echo {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        println!("{}", args);
        (Ok(()), false)
    }
}

struct Help;

impl BuiltinCmd for Help {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        super::builtin_cmd_list(args)
    }
}

struct List;

impl BuiltinCmd for List {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        super::builtin_cmd_list(args)
    }
}

struct Repeat;

impl BuiltinCmd for Repeat {
    fn exec(&self, _args: &str) -> (Result<()>, bool) {
        // XXX: NOP implementation. Due to the way repeat has to work, providing a
        // handler like this is "too late" to be useful. However, a handler
        // is required as "repeat" is a valid command.
        //
        // A cleaner approach would be to make `AgentCmd.fp` an `Option` which for
        // this command would be specified as `None`, but this is the only command
        // which doesn't need an implementation, so this approach is simpler :)

        (Ok(()), false)
    }
}

struct Sleep;

impl BuiltinCmd for Sleep {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        let ns = match utils::human_time_to_ns(args) {
            Ok(t) => t,
            Err(e) => return (Err(e), false),
        };

        sleep(Duration::from_nanos(ns as u64));

        (Ok(()), false)
    }
}

struct Quit;

impl BuiltinCmd for Quit {
    fn exec(&self, _args: &str) -> (Result<()>, bool) {
        (Ok(()), true)
    }
}
