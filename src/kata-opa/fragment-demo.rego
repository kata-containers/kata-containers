# Copyright (c) 2026 Kata Containers community
#
# SPDX-License-Identifier: Apache-2.0
#
# FR-1 signed-policy-fragment demo base policy.
#
# This is `allow-all` for the RPCs needed to boot and run a pod, EXCEPT ExecProcess, which
# is denied by default and only allowed when a signed fragment has contributed the fact
# `data.agent_policy.fragments.exec_allowed == true`. It demonstrates that a verified
# fragment measurably changes an authorization decision at enforcement time (deny -> allow),
# and that a fragment can only contribute within its reserved `agent_policy.fragments`
# namespace (it cannot redefine this base rule).
#
# Used by the FR-1 end-to-end proof (see docs/cc/fr1-fragment-e2e.md).

package agent_policy

default AddARPNeighborsRequest := true
default AddSwapRequest := true
default CloseStdinRequest := true
default CopyFileRequest := true
default CreateContainerRequest := true
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default GetDiagnosticDataRequest := true
default GetMetricsRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default ListInterfacesRequest := true
default ListRoutesRequest := true
default LoadPolicyFragmentRequest := true
default MemAgentCompactConfig := true
default MemAgentMemcgConfig := true
default MemHotplugByProbeRequest := true
default OnlineCPUMemRequest := true
default PauseContainerRequest := true
default PullImageRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default ReseedRandomDevRequest := true
default ResumeContainerRequest := true
default SetGuestDateTimeRequest := true
default SetPolicyRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StartTracingRequest := true
default StatsContainerRequest := true
default StopTracingRequest := true
default TtyWinResizeRequest := true
default UpdateContainerRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true

# Exec is denied unless a signed fragment grants it. The base rule consults the reserved
# fragment namespace; a fragment module (package agent_policy.fragments) sets exec_allowed.
default ExecProcessRequest := false

ExecProcessRequest := data.agent_policy.fragments.exec_allowed
