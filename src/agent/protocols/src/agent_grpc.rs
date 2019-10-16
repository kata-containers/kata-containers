// This file is generated. Do not edit
// @generated

// https://github.com/Manishearth/rust-clippy/issues/702
#![allow(unknown_lints)]
#![allow(clippy)]

#![cfg_attr(rustfmt, rustfmt_skip)]

#![allow(box_pointers)]
#![allow(dead_code)]
#![allow(missing_docs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(trivial_casts)]
#![allow(unsafe_code)]
#![allow(unused_imports)]
#![allow(unused_results)]

const METHOD_AGENT_SERVICE_CREATE_CONTAINER: ::grpcio::Method<super::agent::CreateContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/CreateContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_START_CONTAINER: ::grpcio::Method<super::agent::StartContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/StartContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_REMOVE_CONTAINER: ::grpcio::Method<super::agent::RemoveContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/RemoveContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_EXEC_PROCESS: ::grpcio::Method<super::agent::ExecProcessRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ExecProcess",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_SIGNAL_PROCESS: ::grpcio::Method<super::agent::SignalProcessRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/SignalProcess",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_WAIT_PROCESS: ::grpcio::Method<super::agent::WaitProcessRequest, super::agent::WaitProcessResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/WaitProcess",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_LIST_PROCESSES: ::grpcio::Method<super::agent::ListProcessesRequest, super::agent::ListProcessesResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ListProcesses",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_UPDATE_CONTAINER: ::grpcio::Method<super::agent::UpdateContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/UpdateContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_STATS_CONTAINER: ::grpcio::Method<super::agent::StatsContainerRequest, super::agent::StatsContainerResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/StatsContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_PAUSE_CONTAINER: ::grpcio::Method<super::agent::PauseContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/PauseContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_RESUME_CONTAINER: ::grpcio::Method<super::agent::ResumeContainerRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ResumeContainer",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_WRITE_STDIN: ::grpcio::Method<super::agent::WriteStreamRequest, super::agent::WriteStreamResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/WriteStdin",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_READ_STDOUT: ::grpcio::Method<super::agent::ReadStreamRequest, super::agent::ReadStreamResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ReadStdout",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_READ_STDERR: ::grpcio::Method<super::agent::ReadStreamRequest, super::agent::ReadStreamResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ReadStderr",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_CLOSE_STDIN: ::grpcio::Method<super::agent::CloseStdinRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/CloseStdin",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_TTY_WIN_RESIZE: ::grpcio::Method<super::agent::TtyWinResizeRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/TtyWinResize",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_UPDATE_INTERFACE: ::grpcio::Method<super::agent::UpdateInterfaceRequest, super::types::Interface> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/UpdateInterface",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_UPDATE_ROUTES: ::grpcio::Method<super::agent::UpdateRoutesRequest, super::agent::Routes> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/UpdateRoutes",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_LIST_INTERFACES: ::grpcio::Method<super::agent::ListInterfacesRequest, super::agent::Interfaces> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ListInterfaces",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_LIST_ROUTES: ::grpcio::Method<super::agent::ListRoutesRequest, super::agent::Routes> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ListRoutes",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_START_TRACING: ::grpcio::Method<super::agent::StartTracingRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/StartTracing",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_STOP_TRACING: ::grpcio::Method<super::agent::StopTracingRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/StopTracing",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_CREATE_SANDBOX: ::grpcio::Method<super::agent::CreateSandboxRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/CreateSandbox",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_DESTROY_SANDBOX: ::grpcio::Method<super::agent::DestroySandboxRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/DestroySandbox",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_ONLINE_CPU_MEM: ::grpcio::Method<super::agent::OnlineCPUMemRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/OnlineCPUMem",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_RESEED_RANDOM_DEV: ::grpcio::Method<super::agent::ReseedRandomDevRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/ReseedRandomDev",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_GET_GUEST_DETAILS: ::grpcio::Method<super::agent::GuestDetailsRequest, super::agent::GuestDetailsResponse> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/GetGuestDetails",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_MEM_HOTPLUG_BY_PROBE: ::grpcio::Method<super::agent::MemHotplugByProbeRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/MemHotplugByProbe",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_SET_GUEST_DATE_TIME: ::grpcio::Method<super::agent::SetGuestDateTimeRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/SetGuestDateTime",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

const METHOD_AGENT_SERVICE_COPY_FILE: ::grpcio::Method<super::agent::CopyFileRequest, super::empty::Empty> = ::grpcio::Method {
    ty: ::grpcio::MethodType::Unary,
    name: "/grpc.AgentService/CopyFile",
    req_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
    resp_mar: ::grpcio::Marshaller { ser: ::grpcio::pb_ser, de: ::grpcio::pb_de },
};

#[derive(Clone)]
pub struct AgentServiceClient {
    client: ::grpcio::Client,
}

impl AgentServiceClient {
    pub fn new(channel: ::grpcio::Channel) -> Self {
        AgentServiceClient {
            client: ::grpcio::Client::new(channel),
        }
    }

    pub fn create_container_opt(&self, req: &super::agent::CreateContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_CREATE_CONTAINER, req, opt)
    }

    pub fn create_container(&self, req: &super::agent::CreateContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.create_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn create_container_async_opt(&self, req: &super::agent::CreateContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_CREATE_CONTAINER, req, opt)
    }

    pub fn create_container_async(&self, req: &super::agent::CreateContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.create_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn start_container_opt(&self, req: &super::agent::StartContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_START_CONTAINER, req, opt)
    }

    pub fn start_container(&self, req: &super::agent::StartContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.start_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn start_container_async_opt(&self, req: &super::agent::StartContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_START_CONTAINER, req, opt)
    }

    pub fn start_container_async(&self, req: &super::agent::StartContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.start_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn remove_container_opt(&self, req: &super::agent::RemoveContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_REMOVE_CONTAINER, req, opt)
    }

    pub fn remove_container(&self, req: &super::agent::RemoveContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.remove_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn remove_container_async_opt(&self, req: &super::agent::RemoveContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_REMOVE_CONTAINER, req, opt)
    }

    pub fn remove_container_async(&self, req: &super::agent::RemoveContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.remove_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn exec_process_opt(&self, req: &super::agent::ExecProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_EXEC_PROCESS, req, opt)
    }

    pub fn exec_process(&self, req: &super::agent::ExecProcessRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.exec_process_opt(req, ::grpcio::CallOption::default())
    }

    pub fn exec_process_async_opt(&self, req: &super::agent::ExecProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_EXEC_PROCESS, req, opt)
    }

    pub fn exec_process_async(&self, req: &super::agent::ExecProcessRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.exec_process_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn signal_process_opt(&self, req: &super::agent::SignalProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_SIGNAL_PROCESS, req, opt)
    }

    pub fn signal_process(&self, req: &super::agent::SignalProcessRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.signal_process_opt(req, ::grpcio::CallOption::default())
    }

    pub fn signal_process_async_opt(&self, req: &super::agent::SignalProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_SIGNAL_PROCESS, req, opt)
    }

    pub fn signal_process_async(&self, req: &super::agent::SignalProcessRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.signal_process_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn wait_process_opt(&self, req: &super::agent::WaitProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::WaitProcessResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_WAIT_PROCESS, req, opt)
    }

    pub fn wait_process(&self, req: &super::agent::WaitProcessRequest) -> ::grpcio::Result<super::agent::WaitProcessResponse> {
        self.wait_process_opt(req, ::grpcio::CallOption::default())
    }

    pub fn wait_process_async_opt(&self, req: &super::agent::WaitProcessRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::WaitProcessResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_WAIT_PROCESS, req, opt)
    }

    pub fn wait_process_async(&self, req: &super::agent::WaitProcessRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::WaitProcessResponse>> {
        self.wait_process_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_processes_opt(&self, req: &super::agent::ListProcessesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::ListProcessesResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_LIST_PROCESSES, req, opt)
    }

    pub fn list_processes(&self, req: &super::agent::ListProcessesRequest) -> ::grpcio::Result<super::agent::ListProcessesResponse> {
        self.list_processes_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_processes_async_opt(&self, req: &super::agent::ListProcessesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ListProcessesResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_LIST_PROCESSES, req, opt)
    }

    pub fn list_processes_async(&self, req: &super::agent::ListProcessesRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ListProcessesResponse>> {
        self.list_processes_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_container_opt(&self, req: &super::agent::UpdateContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_UPDATE_CONTAINER, req, opt)
    }

    pub fn update_container(&self, req: &super::agent::UpdateContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.update_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_container_async_opt(&self, req: &super::agent::UpdateContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_UPDATE_CONTAINER, req, opt)
    }

    pub fn update_container_async(&self, req: &super::agent::UpdateContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.update_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn stats_container_opt(&self, req: &super::agent::StatsContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::StatsContainerResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_STATS_CONTAINER, req, opt)
    }

    pub fn stats_container(&self, req: &super::agent::StatsContainerRequest) -> ::grpcio::Result<super::agent::StatsContainerResponse> {
        self.stats_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn stats_container_async_opt(&self, req: &super::agent::StatsContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::StatsContainerResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_STATS_CONTAINER, req, opt)
    }

    pub fn stats_container_async(&self, req: &super::agent::StatsContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::StatsContainerResponse>> {
        self.stats_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn pause_container_opt(&self, req: &super::agent::PauseContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_PAUSE_CONTAINER, req, opt)
    }

    pub fn pause_container(&self, req: &super::agent::PauseContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.pause_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn pause_container_async_opt(&self, req: &super::agent::PauseContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_PAUSE_CONTAINER, req, opt)
    }

    pub fn pause_container_async(&self, req: &super::agent::PauseContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.pause_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn resume_container_opt(&self, req: &super::agent::ResumeContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_RESUME_CONTAINER, req, opt)
    }

    pub fn resume_container(&self, req: &super::agent::ResumeContainerRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.resume_container_opt(req, ::grpcio::CallOption::default())
    }

    pub fn resume_container_async_opt(&self, req: &super::agent::ResumeContainerRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_RESUME_CONTAINER, req, opt)
    }

    pub fn resume_container_async(&self, req: &super::agent::ResumeContainerRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.resume_container_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn write_stdin_opt(&self, req: &super::agent::WriteStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::WriteStreamResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_WRITE_STDIN, req, opt)
    }

    pub fn write_stdin(&self, req: &super::agent::WriteStreamRequest) -> ::grpcio::Result<super::agent::WriteStreamResponse> {
        self.write_stdin_opt(req, ::grpcio::CallOption::default())
    }

    pub fn write_stdin_async_opt(&self, req: &super::agent::WriteStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::WriteStreamResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_WRITE_STDIN, req, opt)
    }

    pub fn write_stdin_async(&self, req: &super::agent::WriteStreamRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::WriteStreamResponse>> {
        self.write_stdin_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn read_stdout_opt(&self, req: &super::agent::ReadStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::ReadStreamResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_READ_STDOUT, req, opt)
    }

    pub fn read_stdout(&self, req: &super::agent::ReadStreamRequest) -> ::grpcio::Result<super::agent::ReadStreamResponse> {
        self.read_stdout_opt(req, ::grpcio::CallOption::default())
    }

    pub fn read_stdout_async_opt(&self, req: &super::agent::ReadStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ReadStreamResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_READ_STDOUT, req, opt)
    }

    pub fn read_stdout_async(&self, req: &super::agent::ReadStreamRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ReadStreamResponse>> {
        self.read_stdout_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn read_stderr_opt(&self, req: &super::agent::ReadStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::ReadStreamResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_READ_STDERR, req, opt)
    }

    pub fn read_stderr(&self, req: &super::agent::ReadStreamRequest) -> ::grpcio::Result<super::agent::ReadStreamResponse> {
        self.read_stderr_opt(req, ::grpcio::CallOption::default())
    }

    pub fn read_stderr_async_opt(&self, req: &super::agent::ReadStreamRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ReadStreamResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_READ_STDERR, req, opt)
    }

    pub fn read_stderr_async(&self, req: &super::agent::ReadStreamRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::ReadStreamResponse>> {
        self.read_stderr_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn close_stdin_opt(&self, req: &super::agent::CloseStdinRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_CLOSE_STDIN, req, opt)
    }

    pub fn close_stdin(&self, req: &super::agent::CloseStdinRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.close_stdin_opt(req, ::grpcio::CallOption::default())
    }

    pub fn close_stdin_async_opt(&self, req: &super::agent::CloseStdinRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_CLOSE_STDIN, req, opt)
    }

    pub fn close_stdin_async(&self, req: &super::agent::CloseStdinRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.close_stdin_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn tty_win_resize_opt(&self, req: &super::agent::TtyWinResizeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_TTY_WIN_RESIZE, req, opt)
    }

    pub fn tty_win_resize(&self, req: &super::agent::TtyWinResizeRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.tty_win_resize_opt(req, ::grpcio::CallOption::default())
    }

    pub fn tty_win_resize_async_opt(&self, req: &super::agent::TtyWinResizeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_TTY_WIN_RESIZE, req, opt)
    }

    pub fn tty_win_resize_async(&self, req: &super::agent::TtyWinResizeRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.tty_win_resize_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_interface_opt(&self, req: &super::agent::UpdateInterfaceRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::types::Interface> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_UPDATE_INTERFACE, req, opt)
    }

    pub fn update_interface(&self, req: &super::agent::UpdateInterfaceRequest) -> ::grpcio::Result<super::types::Interface> {
        self.update_interface_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_interface_async_opt(&self, req: &super::agent::UpdateInterfaceRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::types::Interface>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_UPDATE_INTERFACE, req, opt)
    }

    pub fn update_interface_async(&self, req: &super::agent::UpdateInterfaceRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::types::Interface>> {
        self.update_interface_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_routes_opt(&self, req: &super::agent::UpdateRoutesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::Routes> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_UPDATE_ROUTES, req, opt)
    }

    pub fn update_routes(&self, req: &super::agent::UpdateRoutesRequest) -> ::grpcio::Result<super::agent::Routes> {
        self.update_routes_opt(req, ::grpcio::CallOption::default())
    }

    pub fn update_routes_async_opt(&self, req: &super::agent::UpdateRoutesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Routes>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_UPDATE_ROUTES, req, opt)
    }

    pub fn update_routes_async(&self, req: &super::agent::UpdateRoutesRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Routes>> {
        self.update_routes_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_interfaces_opt(&self, req: &super::agent::ListInterfacesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::Interfaces> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_LIST_INTERFACES, req, opt)
    }

    pub fn list_interfaces(&self, req: &super::agent::ListInterfacesRequest) -> ::grpcio::Result<super::agent::Interfaces> {
        self.list_interfaces_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_interfaces_async_opt(&self, req: &super::agent::ListInterfacesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Interfaces>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_LIST_INTERFACES, req, opt)
    }

    pub fn list_interfaces_async(&self, req: &super::agent::ListInterfacesRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Interfaces>> {
        self.list_interfaces_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_routes_opt(&self, req: &super::agent::ListRoutesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::Routes> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_LIST_ROUTES, req, opt)
    }

    pub fn list_routes(&self, req: &super::agent::ListRoutesRequest) -> ::grpcio::Result<super::agent::Routes> {
        self.list_routes_opt(req, ::grpcio::CallOption::default())
    }

    pub fn list_routes_async_opt(&self, req: &super::agent::ListRoutesRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Routes>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_LIST_ROUTES, req, opt)
    }

    pub fn list_routes_async(&self, req: &super::agent::ListRoutesRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::Routes>> {
        self.list_routes_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn start_tracing_opt(&self, req: &super::agent::StartTracingRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_START_TRACING, req, opt)
    }

    pub fn start_tracing(&self, req: &super::agent::StartTracingRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.start_tracing_opt(req, ::grpcio::CallOption::default())
    }

    pub fn start_tracing_async_opt(&self, req: &super::agent::StartTracingRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_START_TRACING, req, opt)
    }

    pub fn start_tracing_async(&self, req: &super::agent::StartTracingRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.start_tracing_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn stop_tracing_opt(&self, req: &super::agent::StopTracingRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_STOP_TRACING, req, opt)
    }

    pub fn stop_tracing(&self, req: &super::agent::StopTracingRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.stop_tracing_opt(req, ::grpcio::CallOption::default())
    }

    pub fn stop_tracing_async_opt(&self, req: &super::agent::StopTracingRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_STOP_TRACING, req, opt)
    }

    pub fn stop_tracing_async(&self, req: &super::agent::StopTracingRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.stop_tracing_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn create_sandbox_opt(&self, req: &super::agent::CreateSandboxRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_CREATE_SANDBOX, req, opt)
    }

    pub fn create_sandbox(&self, req: &super::agent::CreateSandboxRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.create_sandbox_opt(req, ::grpcio::CallOption::default())
    }

    pub fn create_sandbox_async_opt(&self, req: &super::agent::CreateSandboxRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_CREATE_SANDBOX, req, opt)
    }

    pub fn create_sandbox_async(&self, req: &super::agent::CreateSandboxRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.create_sandbox_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn destroy_sandbox_opt(&self, req: &super::agent::DestroySandboxRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_DESTROY_SANDBOX, req, opt)
    }

    pub fn destroy_sandbox(&self, req: &super::agent::DestroySandboxRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.destroy_sandbox_opt(req, ::grpcio::CallOption::default())
    }

    pub fn destroy_sandbox_async_opt(&self, req: &super::agent::DestroySandboxRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_DESTROY_SANDBOX, req, opt)
    }

    pub fn destroy_sandbox_async(&self, req: &super::agent::DestroySandboxRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.destroy_sandbox_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn online_cpu_mem_opt(&self, req: &super::agent::OnlineCPUMemRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_ONLINE_CPU_MEM, req, opt)
    }

    pub fn online_cpu_mem(&self, req: &super::agent::OnlineCPUMemRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.online_cpu_mem_opt(req, ::grpcio::CallOption::default())
    }

    pub fn online_cpu_mem_async_opt(&self, req: &super::agent::OnlineCPUMemRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_ONLINE_CPU_MEM, req, opt)
    }

    pub fn online_cpu_mem_async(&self, req: &super::agent::OnlineCPUMemRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.online_cpu_mem_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn reseed_random_dev_opt(&self, req: &super::agent::ReseedRandomDevRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_RESEED_RANDOM_DEV, req, opt)
    }

    pub fn reseed_random_dev(&self, req: &super::agent::ReseedRandomDevRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.reseed_random_dev_opt(req, ::grpcio::CallOption::default())
    }

    pub fn reseed_random_dev_async_opt(&self, req: &super::agent::ReseedRandomDevRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_RESEED_RANDOM_DEV, req, opt)
    }

    pub fn reseed_random_dev_async(&self, req: &super::agent::ReseedRandomDevRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.reseed_random_dev_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn get_guest_details_opt(&self, req: &super::agent::GuestDetailsRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::agent::GuestDetailsResponse> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_GET_GUEST_DETAILS, req, opt)
    }

    pub fn get_guest_details(&self, req: &super::agent::GuestDetailsRequest) -> ::grpcio::Result<super::agent::GuestDetailsResponse> {
        self.get_guest_details_opt(req, ::grpcio::CallOption::default())
    }

    pub fn get_guest_details_async_opt(&self, req: &super::agent::GuestDetailsRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::GuestDetailsResponse>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_GET_GUEST_DETAILS, req, opt)
    }

    pub fn get_guest_details_async(&self, req: &super::agent::GuestDetailsRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::agent::GuestDetailsResponse>> {
        self.get_guest_details_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn mem_hotplug_by_probe_opt(&self, req: &super::agent::MemHotplugByProbeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_MEM_HOTPLUG_BY_PROBE, req, opt)
    }

    pub fn mem_hotplug_by_probe(&self, req: &super::agent::MemHotplugByProbeRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.mem_hotplug_by_probe_opt(req, ::grpcio::CallOption::default())
    }

    pub fn mem_hotplug_by_probe_async_opt(&self, req: &super::agent::MemHotplugByProbeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_MEM_HOTPLUG_BY_PROBE, req, opt)
    }

    pub fn mem_hotplug_by_probe_async(&self, req: &super::agent::MemHotplugByProbeRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.mem_hotplug_by_probe_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn set_guest_date_time_opt(&self, req: &super::agent::SetGuestDateTimeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_SET_GUEST_DATE_TIME, req, opt)
    }

    pub fn set_guest_date_time(&self, req: &super::agent::SetGuestDateTimeRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.set_guest_date_time_opt(req, ::grpcio::CallOption::default())
    }

    pub fn set_guest_date_time_async_opt(&self, req: &super::agent::SetGuestDateTimeRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_SET_GUEST_DATE_TIME, req, opt)
    }

    pub fn set_guest_date_time_async(&self, req: &super::agent::SetGuestDateTimeRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.set_guest_date_time_async_opt(req, ::grpcio::CallOption::default())
    }

    pub fn copy_file_opt(&self, req: &super::agent::CopyFileRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<super::empty::Empty> {
        self.client.unary_call(&METHOD_AGENT_SERVICE_COPY_FILE, req, opt)
    }

    pub fn copy_file(&self, req: &super::agent::CopyFileRequest) -> ::grpcio::Result<super::empty::Empty> {
        self.copy_file_opt(req, ::grpcio::CallOption::default())
    }

    pub fn copy_file_async_opt(&self, req: &super::agent::CopyFileRequest, opt: ::grpcio::CallOption) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.client.unary_call_async(&METHOD_AGENT_SERVICE_COPY_FILE, req, opt)
    }

    pub fn copy_file_async(&self, req: &super::agent::CopyFileRequest) -> ::grpcio::Result<::grpcio::ClientUnaryReceiver<super::empty::Empty>> {
        self.copy_file_async_opt(req, ::grpcio::CallOption::default())
    }
    pub fn spawn<F>(&self, f: F) where F: ::futures::Future<Item = (), Error = ()> + Send + 'static {
        self.client.spawn(f)
    }
}

pub trait AgentService {
    fn create_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::CreateContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn start_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::StartContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn remove_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::RemoveContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn exec_process(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ExecProcessRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn signal_process(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::SignalProcessRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn wait_process(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::WaitProcessRequest, sink: ::grpcio::UnarySink<super::agent::WaitProcessResponse>);
    fn list_processes(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ListProcessesRequest, sink: ::grpcio::UnarySink<super::agent::ListProcessesResponse>);
    fn update_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::UpdateContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn stats_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::StatsContainerRequest, sink: ::grpcio::UnarySink<super::agent::StatsContainerResponse>);
    fn pause_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::PauseContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn resume_container(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ResumeContainerRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn write_stdin(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::WriteStreamRequest, sink: ::grpcio::UnarySink<super::agent::WriteStreamResponse>);
    fn read_stdout(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ReadStreamRequest, sink: ::grpcio::UnarySink<super::agent::ReadStreamResponse>);
    fn read_stderr(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ReadStreamRequest, sink: ::grpcio::UnarySink<super::agent::ReadStreamResponse>);
    fn close_stdin(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::CloseStdinRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn tty_win_resize(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::TtyWinResizeRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn update_interface(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::UpdateInterfaceRequest, sink: ::grpcio::UnarySink<super::types::Interface>);
    fn update_routes(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::UpdateRoutesRequest, sink: ::grpcio::UnarySink<super::agent::Routes>);
    fn list_interfaces(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ListInterfacesRequest, sink: ::grpcio::UnarySink<super::agent::Interfaces>);
    fn list_routes(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ListRoutesRequest, sink: ::grpcio::UnarySink<super::agent::Routes>);
    fn start_tracing(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::StartTracingRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn stop_tracing(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::StopTracingRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn create_sandbox(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::CreateSandboxRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn destroy_sandbox(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::DestroySandboxRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn online_cpu_mem(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::OnlineCPUMemRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn reseed_random_dev(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::ReseedRandomDevRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn get_guest_details(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::GuestDetailsRequest, sink: ::grpcio::UnarySink<super::agent::GuestDetailsResponse>);
    fn mem_hotplug_by_probe(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::MemHotplugByProbeRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn set_guest_date_time(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::SetGuestDateTimeRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
    fn copy_file(&mut self, ctx: ::grpcio::RpcContext, req: super::agent::CopyFileRequest, sink: ::grpcio::UnarySink<super::empty::Empty>);
}

pub fn create_agent_service<S: AgentService + Send + Clone + 'static>(s: S) -> ::grpcio::Service {
    let mut builder = ::grpcio::ServiceBuilder::new();
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_CREATE_CONTAINER, move |ctx, req, resp| {
        instance.create_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_START_CONTAINER, move |ctx, req, resp| {
        instance.start_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_REMOVE_CONTAINER, move |ctx, req, resp| {
        instance.remove_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_EXEC_PROCESS, move |ctx, req, resp| {
        instance.exec_process(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_SIGNAL_PROCESS, move |ctx, req, resp| {
        instance.signal_process(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_WAIT_PROCESS, move |ctx, req, resp| {
        instance.wait_process(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_LIST_PROCESSES, move |ctx, req, resp| {
        instance.list_processes(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_UPDATE_CONTAINER, move |ctx, req, resp| {
        instance.update_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_STATS_CONTAINER, move |ctx, req, resp| {
        instance.stats_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_PAUSE_CONTAINER, move |ctx, req, resp| {
        instance.pause_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_RESUME_CONTAINER, move |ctx, req, resp| {
        instance.resume_container(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_WRITE_STDIN, move |ctx, req, resp| {
        instance.write_stdin(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_READ_STDOUT, move |ctx, req, resp| {
        instance.read_stdout(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_READ_STDERR, move |ctx, req, resp| {
        instance.read_stderr(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_CLOSE_STDIN, move |ctx, req, resp| {
        instance.close_stdin(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_TTY_WIN_RESIZE, move |ctx, req, resp| {
        instance.tty_win_resize(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_UPDATE_INTERFACE, move |ctx, req, resp| {
        instance.update_interface(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_UPDATE_ROUTES, move |ctx, req, resp| {
        instance.update_routes(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_LIST_INTERFACES, move |ctx, req, resp| {
        instance.list_interfaces(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_LIST_ROUTES, move |ctx, req, resp| {
        instance.list_routes(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_START_TRACING, move |ctx, req, resp| {
        instance.start_tracing(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_STOP_TRACING, move |ctx, req, resp| {
        instance.stop_tracing(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_CREATE_SANDBOX, move |ctx, req, resp| {
        instance.create_sandbox(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_DESTROY_SANDBOX, move |ctx, req, resp| {
        instance.destroy_sandbox(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_ONLINE_CPU_MEM, move |ctx, req, resp| {
        instance.online_cpu_mem(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_RESEED_RANDOM_DEV, move |ctx, req, resp| {
        instance.reseed_random_dev(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_GET_GUEST_DETAILS, move |ctx, req, resp| {
        instance.get_guest_details(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_MEM_HOTPLUG_BY_PROBE, move |ctx, req, resp| {
        instance.mem_hotplug_by_probe(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_SET_GUEST_DATE_TIME, move |ctx, req, resp| {
        instance.set_guest_date_time(ctx, req, resp)
    });
    let mut instance = s.clone();
    builder = builder.add_unary_handler(&METHOD_AGENT_SERVICE_COPY_FILE, move |ctx, req, resp| {
        instance.copy_file(ctx, req, resp)
    });
    builder.build()
}
