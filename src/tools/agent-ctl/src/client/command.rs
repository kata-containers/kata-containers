use crate::types::Options;
use anyhow::Result;
use protocols::{agent_ttrpc::AgentServiceClient, health_ttrpc::HealthClient};
use ttrpc::context::Context;

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
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct AddSwap;

impl AgentCmd for AddSwap {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct Check;

impl AgentCmd for Check {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct Version;

impl AgentCmd for Version {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct CloseStdin;

impl AgentCmd for CloseStdin {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct CopyFile;

impl AgentCmd for CopyFile {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct CreateContainer;

impl AgentCmd for CreateContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct CreateSandbox;

impl AgentCmd for CreateSandbox {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct DestroySandbox;

impl AgentCmd for DestroySandbox {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ExecProcess;

impl AgentCmd for ExecProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct GetGuestDetails;

impl AgentCmd for GetGuestDetails {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct GetIptables;

impl AgentCmd for GetIptables {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct GetMetrics;

impl AgentCmd for GetMetrics {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct GetOOMEvent;

impl AgentCmd for GetOOMEvent {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct GetVolumeStats;

impl AgentCmd for GetVolumeStats {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ListInterfaces;

impl AgentCmd for ListInterfaces {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ListRoutes;

impl AgentCmd for ListRoutes {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct MemHotplugByProbe;

impl AgentCmd for MemHotplugByProbe {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct OnlineCPUMem;

impl AgentCmd for OnlineCPUMem {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct PauseContainer;

impl AgentCmd for PauseContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ReadStderr;

impl AgentCmd for ReadStderr {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ReadStdout;

impl AgentCmd for ReadStdout {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ReseedRandomDev;

impl AgentCmd for ReseedRandomDev {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct RemoveContainer;

impl AgentCmd for RemoveContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct ResumeContainer;

impl AgentCmd for ResumeContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct SetGuestDateTime;

impl AgentCmd for SetGuestDateTime {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct SetIptables;

impl AgentCmd for SetIptables {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct SignalProcess;

impl AgentCmd for SignalProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct StartContainer;

impl AgentCmd for StartContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct StatsContainer;

impl AgentCmd for StatsContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct TtyWinResize;

impl AgentCmd for TtyWinResize {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct UpdateContainer;

impl AgentCmd for UpdateContainer {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct UpdateInterface;

impl AgentCmd for UpdateInterface {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct UpdateRoutes;

impl AgentCmd for UpdateRoutes {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct WaitProcess;

impl AgentCmd for WaitProcess {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

struct WriteStdin;

impl AgentCmd for WriteStdin {
    fn exec(
        &self,
        ctx: &Context,
        client: &AgentServiceClient,
        health: &HealthClient,
        options: &mut Options,
        args: &str,
    ) -> (Result<()>, bool) {
        todo!()
    }
}

pub trait BuiltinCmd {
    fn exec(&self, args: &str) -> (Result<()>, bool);
}

struct Echo;

impl BuiltinCmd for Echo {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}

struct Help;

impl BuiltinCmd for Help {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}

struct List;

impl BuiltinCmd for List {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}

struct Repeat;

impl BuiltinCmd for Repeat {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}

struct Sleep;

impl BuiltinCmd for Sleep {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}

struct Quit;

impl BuiltinCmd for Quit {
    fn exec(&self, args: &str) -> (Result<()>, bool) {
        todo!()
    }
}
