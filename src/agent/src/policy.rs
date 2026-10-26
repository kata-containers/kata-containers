// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use protobuf::MessageDyn;
use tokio::sync::MutexGuard;

use crate::rpc::ttrpc_error;
use crate::{AGENT_POLICY, POLICY_STATE_LOCK};
use kata_agent_policy::policy::{AgentPolicy, PolicyDecision};

async fn allow_request(policy: &mut AgentPolicy, ep: &str, request: &str) -> ttrpc::Result<()> {
    match policy.allow_request(ep, request).await {
        Ok((allowed, prints)) => {
            if allowed {
                Ok(())
            } else {
                Err(ttrpc_error(
                    ttrpc::Code::PERMISSION_DENIED,
                    format!("{ep} is blocked by policy: {prints}"),
                ))
            }
        }
        Err(e) => Err(ttrpc_error(
            ttrpc::Code::INTERNAL,
            format!("{ep}: internal error {e}"),
        )),
    }
}

/// is_allowed() is used for ttrpc request types that *are not* expected to access
/// (read and/or write) the policy state. The policy state lock doesn't get acquired on this
/// code path, so an incorrect rego policy reading the state for one of these requests might
/// silently encounter incorrect state data, make incorrect policy decisions, etc. See the
/// description of is_allowed_stateful() for the list of request types that are expected to
/// use state.
pub async fn is_allowed(req: &(impl MessageDyn + serde::Serialize)) -> ttrpc::Result<()> {
    is_allowed_with_entrypoint(req.descriptor_dyn().name(), &req).await
}

pub struct PolicyStateGuard {
    _state_lock: MutexGuard<'static, ()>,
    decision: PolicyDecision,
}

impl PolicyStateGuard {
    /// commit() is used after is_allowed_stateful() and after successfully processing
    /// a ttrpc request. It applies to the policy state data the patch returned by the
    /// rego policy, if there was such a patch.
    pub async fn commit(self) -> ttrpc::Result<()> {
        let mut policy = AGENT_POLICY.lock().await;
        self.decision.commit(&mut policy).await.map_err(|e| {
            ttrpc_error(
                ttrpc::Code::INTERNAL,
                format!("failed to commit policy state: {e}"),
            )
        })
    }
}

/// is_allowed_stateful() is used for ttrpc request types that *are* expected to access
/// (read and/or write) the policy state. In the current design, these requests are:
///
/// - CreateContainerRequest
/// - CreateSandboxRequest
/// - ExecProcessRequest
/// - RemoveContainerRequest
///
/// E.g., a typical caller's code flow is:
///
/// let policy_state = is_allowed_stateful(&req).await?;
/// self.do_create_container(req).await.map_ttrpc_err(same)?;
/// policy_state.commit().await?;
pub async fn is_allowed_stateful(
    req: &(impl MessageDyn + serde::Serialize),
) -> ttrpc::Result<PolicyStateGuard> {
    let state_lock = POLICY_STATE_LOCK.lock().await;
    let descriptor = req.descriptor_dyn();
    let ep = descriptor.name();
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    let decision = policy
        .evaluate_request(ep, &request)
        .await
        .map_err(|e| ttrpc_error(ttrpc::Code::INTERNAL, format!("{ep}: internal error {e}")))?;

    if !decision.allowed() {
        return Err(ttrpc_error(
            ttrpc::Code::PERMISSION_DENIED,
            format!("{ep} is blocked by policy: {}", decision.prints()),
        ));
    }

    Ok(PolicyStateGuard {
        _state_lock: state_lock,
        decision,
    })
}

/// is_allowed_with_entrypoint() is used for ttrpc request types that *are not* expected to access
/// (read and/or write) the policy state. The policy state lock doesn't get acquired on this
/// code path, so an incorrect rego policy reading the state for one of these requests might
/// silently encounter incorrect state data, make incorrect policy decisions, etc. See the
/// description of is_allowed_stateful() for the list of request types that are expected to
/// use state.
pub async fn is_allowed_with_entrypoint(
    ep: &str,
    req: &impl serde::Serialize,
) -> ttrpc::Result<()> {
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, ep, &request).await
}

/// do_set_policy() replaces the current policy, including the policy state data. So, it acquires
/// the state lock in addition to the policy evaluation lock.
pub async fn do_set_policy(req: &protocols::agent::SetPolicyRequest) -> ttrpc::Result<()> {
    let _state_lock = POLICY_STATE_LOCK.lock().await;
    let request = serde_json::to_string(req).unwrap();
    let mut policy = AGENT_POLICY.lock().await;
    allow_request(&mut policy, "SetPolicyRequest", &request).await?;
    policy
        .set_policy(&req.policy)
        .await
        .map_err(|e| ttrpc_error(ttrpc::Code::INVALID_ARGUMENT, e))
}
