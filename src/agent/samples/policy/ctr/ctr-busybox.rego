package coco_policy

import future.keywords.in
import future.keywords.every

import input
import data.coco

######################################################################
# Default values:
#
# - true for requests that are allowed by default.
# - false for requests that have additional policy rules, defined below.
# - Requests that are not listed here get rejected by default.

# More detailed policy rules are below.
default CreateContainerRequest := false

# Requests that are always allowed.
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default OnlineCPUMemRequest := true
default ReadStreamRequest := true
default RemoveContainerRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StatsContainerRequest := true
default TtyWinResizeRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := true


# Image service should make is_allowed!() calls.
#
# Might use policy metadata to reject images that were
# not referenced by config.json.
#default PullImageRequest := false


######################################################################
# Could check that "terminal": true.

CreateContainerRequest {
    policy_container := coco.policy_containers[0]
    input_container := input.oci

    policy_container.ociVersion     == input_container.ociVersion

    allow_annotations(policy_container, input_container)

    policy_process := policy_container.process
    input_process := input_container.process

    policy_process.terminal         == input_process.terminal
    policy_process.user             == input_process.user
    policy_process.args             == input_process.args

    # Ignore any policy environment variables that are not
    # present in the input.
    every env_var in input_process.env {
        policy_process.env[_] == env_var
    }

    policy_process.cwd              == input_process.cwd
    policy_process.capabilities     == input_process.capabilities
    policy_process.rlimits          == input_process.rlimits
    policy_process.noNewPrivileges  == input_process.noNewPrivileges
    policy_process.oomScoreAdj      == input_process.oomScoreAdj
   
    regex.match(policy_container.root.path, input_container.root.path)
    policy_container.root.readonly  == input_container.root.readonly

    policy_container.mounts         == input_container.mounts
    allow_linux(policy_container, input_container)
}

######################################################################
# No annotations allowed for ctr based containers.

allow_annotations(policy_container, input_container) {
    not policy_container.annotations
    not input_container.annotations
}

######################################################################
# linux fields

allow_linux(policy_container, input_container) {
    policy_container.linux.namespaces == input_container.linux.namespaces
    policy_container.linux.maskedPaths == input_container.linux.maskedPaths
    policy_container.linux.readonlyPaths == input_container.linux.readonlyPaths
}
