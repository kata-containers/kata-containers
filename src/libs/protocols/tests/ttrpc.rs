// SPDX-License-Identifier: Apache-2.0

#![cfg(feature = "async")]

use std::sync::Arc;
use std::time::Duration;

use protobuf::well_known_types::empty::Empty;
use protocols::agent::PauseContainerRequest;
use protocols::{agent_ttrpc, agent_ttrpc_async};
use ttrpc::{context, Code};

struct AgentService;

fn pause(request: PauseContainerRequest) -> ttrpc::Result<Empty> {
    if request.container_id != "test-container" {
        return Err(ttrpc::Error::RpcStatus(ttrpc::get_status(
            Code::INVALID_ARGUMENT,
            "unknown container".to_owned(),
        )));
    }
    Ok(Empty::new())
}

impl agent_ttrpc::AgentService for AgentService {
    fn pause_container(
        &self,
        _ctx: &ttrpc::TtrpcContext,
        request: PauseContainerRequest,
    ) -> ttrpc::Result<Empty> {
        pause(request)
    }
}

#[async_trait::async_trait]
impl agent_ttrpc_async::AgentService for AgentService {
    async fn pause_container(
        &self,
        _ctx: &ttrpc::asynchronous::TtrpcContext,
        request: PauseContainerRequest,
    ) -> ttrpc::Result<Empty> {
        pause(request)
    }
}

async fn check_clients(address: String) {
    let client = agent_ttrpc_async::AgentServiceClient::new(
        ttrpc::asynchronous::Client::connect(&address)
            .await
            .unwrap(),
    );
    let request = PauseContainerRequest {
        container_id: "test-container".to_owned(),
        ..Default::default()
    };
    let response: Empty = client
        .pause_container(context::with_duration(Duration::from_secs(2)), &request)
        .await
        .unwrap();
    assert_eq!(response, Empty::new());

    let error = client
        .pause_container(
            context::with_duration(Duration::from_secs(2)),
            &PauseContainerRequest::new(),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, ttrpc::Error::RpcStatus(status)
        if status.code() == Code::INVALID_ARGUMENT));
    drop(client);

    tokio::task::spawn_blocking(move || {
        let client =
            agent_ttrpc::AgentServiceClient::new(ttrpc::Client::connect(&address).unwrap());
        let response: Empty = client
            .pause_container(context::with_duration(Duration::from_secs(2)), &request)
            .unwrap();
        assert_eq!(response, Empty::new());
        let error = client
            .pause_container(
                context::with_duration(Duration::from_secs(2)),
                &PauseContainerRequest::new(),
            )
            .unwrap_err();
        assert!(matches!(error, ttrpc::Error::RpcStatus(status)
            if status.code() == Code::INVALID_ARGUMENT));
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_and_async_clients_use_wkt_with_async_server() {
    let directory = tempfile::tempdir().unwrap();
    let address = format!("unix://{}", directory.path().join("agent.sock").display());
    let mut server = ttrpc::asynchronous::Server::new()
        .bind(&address)
        .unwrap()
        .register_service(agent_ttrpc_async::create_agent_service(Arc::new(
            AgentService,
        )));
    server.start().await.unwrap();

    check_clients(address).await;

    server.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_and_async_clients_use_wkt_with_sync_server() {
    let directory = tempfile::tempdir().unwrap();
    let address = format!("unix://{}", directory.path().join("agent.sock").display());
    let mut server = ttrpc::Server::new()
        .bind(&address)
        .unwrap()
        .register_service(agent_ttrpc::create_agent_service(Arc::new(AgentService)));
    server.start().unwrap();

    check_clients(address).await;

    server.shutdown();
}
