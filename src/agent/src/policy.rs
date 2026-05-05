// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use protobuf::MessageDyn;

use crate::rpc::ttrpc_error;
use crate::AGENT_POLICY;
use kata_agent_policy::policy::AgentPolicy;
use protocols::oci::User;

async fn allow_request(policy: &mut AgentPolicy, ep: &str, request: &str) -> ttrpc::Result<()> {
    allow_request_with_metadata(policy, ep, request)
        .await
        .map(|_| ())
}

async fn allow_request_with_metadata(
    policy: &mut AgentPolicy,
    ep: &str,
    request: &str,
) -> ttrpc::Result<Option<User>> {
    match policy.allow_request_with_metadata(ep, request).await {
        Ok(response) if response.allowed => Ok(response.policy_user),
        Ok(response) => Err(ttrpc_error(
            ttrpc::Code::PERMISSION_DENIED,
            format!("{ep} is blocked by policy: {}", response.prints),
        )),
        Err(e) => Err(ttrpc_error(
            ttrpc::Code::INTERNAL,
            format!("{ep}: internal error {e}"),
        )),
    }
}

pub async fn is_allowed(req: &(impl MessageDyn + serde::Serialize)) -> ttrpc::Result<()> {
    is_allowed_with_entrypoint(req.descriptor_dyn().name(), &req).await
}

pub async fn is_allowed_with_policy_user(
    req: &(impl MessageDyn + serde::Serialize),
) -> ttrpc::Result<Option<User>> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request_with_metadata(&mut policy, req.descriptor_dyn().name(), &request).await
}

pub async fn is_allowed_with_entrypoint(
    ep: &str,
    req: &impl serde::Serialize,
) -> ttrpc::Result<()> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, ep, &request).await
}

pub async fn do_set_policy(req: &protocols::agent::SetPolicyRequest) -> ttrpc::Result<()> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, "SetPolicyRequest", &request).await?;
    policy
        .set_policy(&req.policy)
        .await
        .map_err(|e| ttrpc_error(ttrpc::Code::INVALID_ARGUMENT, e))
}
