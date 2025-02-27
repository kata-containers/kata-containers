# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
package agent_policy

import future.keywords.in
import future.keywords.every

# Default values, returned by OPA when rules cannot be evaluated to true.
default AddARPNeighborsRequest := false
default AddSwapRequest := false
default CloseStdinRequest := false
default CopyFileRequest := false
default CreateContainerRequest := false
default CreateSandboxRequest := false
default DestroySandboxRequest := true
default ExecProcessRequest := false
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default ListInterfacesRequest := false
default ListRoutesRequest := false
default MemHotplugByProbeRequest := false
default OnlineCPUMemRequest := true
default PauseContainerRequest := false
default ReadStreamRequest := false
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default ReseedRandomDevRequest := false
default ResumeContainerRequest := false
default SetGuestDateTimeRequest := false
default SetPolicyRequest := false
default SignalProcessRequest := true
default StartContainerRequest := true
default StartTracingRequest := false
default StatsContainerRequest := true
default StopTracingRequest := false
default TtyWinResizeRequest := true
default UpdateContainerRequest := false
default UpdateEphemeralMountsRequest := false
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true
default WriteStreamRequest := false

# AllowRequestsFailingPolicy := true configures the Agent to *allow any
# requests causing a policy failure*. This is an unsecure configuration
# but is useful for allowing unsecure pods to start, then connect to
# them and inspect OPA logs for the root cause of a failure.
default AllowRequestsFailingPolicy := false

# Constants
S_NAME_KEY = "io.kubernetes.cri.sandbox-name"
S_NAMESPACE_KEY = "io.kubernetes.cri.sandbox-namespace"

CreateContainerRequest:= {"ops": ops, "allowed": true} {
    # Check if the input request should be rejected even before checking the
    # policy_data.containers information.
    allow_create_container_input

    i_oci := input.OCI
    i_storages := input.storages
    i_devices := input.devices

    # array of possible state operations
    ops_builder := []

    # check sandbox name
    sandbox_name = i_oci.Annotations[S_NAME_KEY]
    add_sandbox_name_to_state := state_allows("sandbox_name", sandbox_name)
    ops_builder1 := concat_op_if_not_null(ops_builder, add_sandbox_name_to_state)

    # Check if any element from the policy_data.containers array allows the input request.
    some p_container in policy_data.containers
    print("======== CreateContainerRequest: trying next policy container")

    p_pidns := p_container.sandbox_pidns
    i_pidns := input.sandbox_pidns
    print("CreateContainerRequest: p_pidns =", p_pidns, "i_pidns =", i_pidns)
    p_pidns == i_pidns

    p_oci := p_container.OCI

    # check namespace
    p_namespace := p_oci.Annotations[S_NAMESPACE_KEY]
    i_namespace := i_oci.Annotations[S_NAMESPACE_KEY]
    print ("CreateContainerRequest: p_namespace =", p_namespace, "i_namespace =", i_namespace)
    add_namespace_to_state := allow_namespace(p_namespace, i_namespace)
    ops_builder2 := concat_op_if_not_null(ops_builder1, add_namespace_to_state)

    print("CreateContainerRequest: p Version =", p_oci.Version, "i Version =", i_oci.Version)
    p_oci.Version == i_oci.Version

    print("CreateContainerRequest: p Readonly =", p_oci.Root.Readonly, "i Readonly =", i_oci.Root.Readonly)
    p_oci.Root.Readonly == i_oci.Root.Readonly

    allow_anno(p_oci, i_oci)

    p_storages := p_container.storages
    allow_by_anno(p_oci, i_oci, p_storages, i_storages)

    p_devices := p_container.devices
    allow_devices(p_devices, i_devices)

    ret := allow_linux(ops_builder2, p_oci, i_oci)
    ret.allowed

    ops := ret.ops

    print("CreateContainerRequest: true")
}

allow_create_container_input {
    print("allow_create_container_input: input =", input)

    count(input.shared_mounts) == 0
    is_null(input.string_user)

    i_oci := input.OCI
    is_null(i_oci.Hooks)
    is_null(i_oci.Solaris)
    is_null(i_oci.Windows)

    i_linux := i_oci.Linux
    count(i_linux.GIDMappings) == 0
    count(i_linux.MountLabel) == 0
    count(i_linux.Resources.Devices) == 0
    count(i_linux.RootfsPropagation) == 0
    count(i_linux.UIDMappings) == 0
    is_null(i_linux.IntelRdt)
    is_null(i_linux.Resources.BlockIO)
    is_null(i_linux.Resources.Network)
    is_null(i_linux.Resources.Pids)
    is_null(i_linux.Seccomp)
    i_linux.Sysctl == {}

    i_process := i_oci.Process
    count(i_process.SelinuxLabel) == 0
    count(i_process.User.Username) == 0

    print("allow_create_container_input: true")
}

allow_namespace(p_namespace, i_namespace) = add_namespace {
    p_namespace == i_namespace
    add_namespace := null
    print("allow_namespace 1: input namespace matches policy data")
}

allow_namespace(p_namespace, i_namespace) = add_namespace {
    p_namespace == ""
    print("allow_namespace 2: no namespace found on policy data")
    add_namespace := state_allows("namespace", i_namespace)
}

# key hasn't been seen before, save key, value pair to state
state_allows(key, value) = action {
  state := get_state()
  print("state_allows 1: state[key] =", state[key], "value =", value)
  not state[key]
  print("state_allows 1: saving to state key =", key, "value =", value)
  path := get_state_path(key) 
  action := {
    "op": "add",
    "path": path, 
    "value": value,
  }
}

# value matches what's in state, allow it
state_allows(key, value) = action {
  print("state_allows 2: start")
  state := get_state()
  print("state_allows 2: state[key] =", state[key], "value =", value)
  value == state[key]
  print("state_allows 2: found key =", key, "value =", value, " in state")
  action := null
}

# helper functions to interact with the state
get_state() = state {
  state := data["pstate"]
}

get_state_path(key) = path {
    # prepend "/pstate/" to key
    path := concat("/", ["/pstate", key])
}

# Helper functions to conditionally concatenate op is not null
concat_op_if_not_null(ops, op) = result {
    op == null
    result := ops
}

concat_op_if_not_null(ops, op) = result {
    op != null
    result := array.concat(ops, [op])
}

# Reject unexpected annotations.
allow_anno(p_oci, i_oci) {
    print("allow_anno 1: start")

    not i_oci.Annotations

    print("allow_anno 1: true")
}
allow_anno(p_oci, i_oci) {
    print("allow_anno 2: p Annotations =", p_oci.Annotations)
    print("allow_anno 2: i Annotations =", i_oci.Annotations)

    i_keys := object.keys(i_oci.Annotations)
    print("allow_anno 2: i keys =", i_keys)

    every i_key in i_keys {
        allow_anno_key(i_key, p_oci)
    }

    print("allow_anno 2: true")
}

allow_anno_key(i_key, p_oci) {
    print("allow_anno_key 1: i key =", i_key)

    startswith(i_key, "io.kubernetes.cri.")

    print("allow_anno_key 1: true")
}
allow_anno_key(i_key, p_oci) {
    print("allow_anno_key 2: i key =", i_key)

    some p_key, _ in p_oci.Annotations
    p_key == i_key

    print("allow_anno_key 2: true")
}

# Get the value of the S_NAME_KEY annotation and
# correlate it with other annotations and process fields.
allow_by_anno(p_oci, i_oci, p_storages, i_storages) {
    print("allow_by_anno 1: start")

    not p_oci.Annotations[S_NAME_KEY]

    i_s_name := i_oci.Annotations[S_NAME_KEY]
    print("allow_by_anno 1: i_s_name =", i_s_name)

    allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, i_s_name)

    print("allow_by_anno 1: true")
}
allow_by_anno(p_oci, i_oci, p_storages, i_storages) {
    print("allow_by_anno 2: start")

    p_s_name := p_oci.Annotations[S_NAME_KEY]
    i_s_name := i_oci.Annotations[S_NAME_KEY]
    print("allow_by_anno 2: i_s_name =", i_s_name, "p_s_name =", p_s_name)

    allow_sandbox_name(p_s_name, i_s_name)
    allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, i_s_name)

    print("allow_by_anno 2: true")
}

allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, s_name) {
    print("allow_by_sandbox_name: start")

    i_namespace := i_oci.Annotations[S_NAMESPACE_KEY]

    allow_by_container_types(p_oci, i_oci, s_name, i_namespace)
    allow_by_bundle_or_sandbox_id(p_oci, i_oci, p_storages, i_storages)
    allow_process(p_oci, i_oci, s_name)

    print("allow_by_sandbox_name: true")
}

allow_sandbox_name(p_s_name, i_s_name) {
    print("allow_sandbox_name 1: start")

    p_s_name == i_s_name

    print("allow_sandbox_name 1: true")
}
allow_sandbox_name(p_s_name, i_s_name) {
    print("allow_sandbox_name 2: start")

    # TODO: should generated names be handled differently?
    contains(p_s_name, "$(generated-name)")

    print("allow_sandbox_name 2: true")
}

# Check that the "io.kubernetes.cri.container-type" and
# "io.katacontainers.pkg.oci.container_type" annotations designate the
# expected type - either a "sandbox" or a "container". Then, validate
# other annotations based on the actual "sandbox" or "container" value
# from the input container.
allow_by_container_types(p_oci, i_oci, s_name, s_namespace) {
    print("allow_by_container_types: checking io.kubernetes.cri.container-type")

    c_type := "io.kubernetes.cri.container-type"
    
    p_cri_type := p_oci.Annotations[c_type]
    i_cri_type := i_oci.Annotations[c_type]
    print("allow_by_container_types: p_cri_type =", p_cri_type, "i_cri_type =", i_cri_type)
    p_cri_type == i_cri_type

    allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace)

    print("allow_by_container_types: true")
}

allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace) {
    print("allow_by_container_type 1: i_cri_type =", i_cri_type)
    i_cri_type == "sandbox"

    i_kata_type := i_oci.Annotations["io.katacontainers.pkg.oci.container_type"]
    print("allow_by_container_type 1: i_kata_type =", i_kata_type)
    i_kata_type == "pod_sandbox"

    allow_sandbox_container_name(p_oci, i_oci)
    allow_sandbox_net_namespace(p_oci, i_oci)
    allow_sandbox_log_directory(p_oci, i_oci, s_name, s_namespace)

    print("allow_by_container_type 1: true")
}

allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace) {
    print("allow_by_container_type 2: i_cri_type =", i_cri_type)
    i_cri_type == "container"

    i_kata_type := i_oci.Annotations["io.katacontainers.pkg.oci.container_type"]
    print("allow_by_container_type 2: i_kata_type =", i_kata_type)
    i_kata_type == "pod_container"

    allow_container_name(p_oci, i_oci)
    allow_net_namespace(p_oci, i_oci)
    allow_log_directory(p_oci, i_oci)

    print("allow_by_container_type 2: true")
}

# "io.kubernetes.cri.container-name" annotation
allow_sandbox_container_name(p_oci, i_oci) {
    print("allow_sandbox_container_name: start")

    container_annotation_missing(p_oci, i_oci, "io.kubernetes.cri.container-name")

    print("allow_sandbox_container_name: true")
}

allow_container_name(p_oci, i_oci) {
    print("allow_container_name: start")

    allow_container_annotation(p_oci, i_oci, "io.kubernetes.cri.container-name")

    print("allow_container_name: true")
}

container_annotation_missing(p_oci, i_oci, key) {
    print("container_annotation_missing:", key)

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("container_annotation_missing: true")
}

allow_container_annotation(p_oci, i_oci, key) {
    print("allow_container_annotation: key =", key)

    p_value := p_oci.Annotations[key]
    i_value := i_oci.Annotations[key]
    print("allow_container_annotation: p_value =", p_value, "i_value =", i_value)

    p_value == i_value

    print("allow_container_annotation: true")
}

# "nerdctl/network-namespace" annotation
allow_sandbox_net_namespace(p_oci, i_oci) {
    print("allow_sandbox_net_namespace: start")

    key := "nerdctl/network-namespace"

    p_namespace := p_oci.Annotations[key]
    i_namespace := i_oci.Annotations[key]
    print("allow_sandbox_net_namespace: p_namespace =", p_namespace, "i_namespace =", i_namespace)

    regex.match(p_namespace, i_namespace)

    print("allow_sandbox_net_namespace: true")
}

allow_net_namespace(p_oci, i_oci) {
    print("allow_net_namespace: start")

    key := "nerdctl/network-namespace"

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("allow_net_namespace: true")
}

# "io.kubernetes.cri.sandbox-log-directory" annotation
allow_sandbox_log_directory(p_oci, i_oci, s_name, s_namespace) {
    print("allow_sandbox_log_directory: start")

    key := "io.kubernetes.cri.sandbox-log-directory"

    p_dir := p_oci.Annotations[key]
    regex1 := replace(p_dir, "$(sandbox-name)", s_name)
    regex2 := replace(regex1, "$(sandbox-namespace)", s_namespace)
    print("allow_sandbox_log_directory: regex2 =", regex2)

    i_dir := i_oci.Annotations[key]
    print("allow_sandbox_log_directory: i_dir =", i_dir)

    regex.match(regex2, i_dir)

    print("allow_sandbox_log_directory: true")
}

allow_log_directory(p_oci, i_oci) {
    print("allow_log_directory: start")

    key := "io.kubernetes.cri.sandbox-log-directory"

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("allow_log_directory: true")
}

allow_devices(p_devices, i_devices) {
    print("allow_devices: start")
    every i_device in i_devices {
        print("allow_devices: i_device =", i_device)
        some p_device in p_devices
        p_device.container_path == i_device.container_path
    }
    print("allow_devices: true")
}

allow_linux(state_ops, p_oci, i_oci) := {"ops": ops, "allowed": true} {
    p_namespaces := p_oci.Linux.Namespaces
    print("allow_linux: p namespaces =", p_namespaces)

    i_namespaces := i_oci.Linux.Namespaces
    print("allow_linux: i namespaces =", i_namespaces)

    i_namespace_without_network := [obj | obj := i_namespaces[_]; obj.Type != "network"]

    print("allow_linux: i_namespace_without_network =", i_namespace_without_network)

    p_namespaces == i_namespace_without_network

    allow_masked_paths(p_oci, i_oci)
    allow_readonly_paths(p_oci, i_oci)
    allow_linux_devices(p_oci.Linux.Devices, i_oci.Linux.Devices)
    ret := allow_network_namespace_start(state_ops, p_oci, i_oci)
    ret.allowed

    ops := ret.ops

    print("allow_linux: true")
}

# Retrieve the "network" namespace from the input data and pass it on for the
# network namespace policy checks.
allow_network_namespace_start(state_ops, p_oci, i_oci) := {"ops": ops, "allowed": true} {
    print("allow_network_namespace start: start")

    p_namespaces := p_oci.Linux.Namespaces
    print("allow_network_namespace start: p namespaces =", p_namespaces)

    i_namespaces := i_oci.Linux.Namespaces
    print("allow_network_namespace start: i namespaces =", i_namespaces)

    # Return path of the "network" namespace
    network_ns := [obj | obj := i_namespaces[_]; obj.Type == "network"]

    print("allow_network_namespace start: network_ns =", network_ns)

    ret := allow_network_namespace(state_ops, network_ns)
    ret.allowed

    ops := ret.ops
}

# This rule is when there's no network namespace in the input data.
allow_network_namespace(state_ops, network_ns) := {"ops": ops, "allowed": true} {
    count(network_ns) == 0

    network_ns_path = ""

    add_network_namespace_to_state := state_allows("network_namespace", network_ns_path)
    ops := concat_op_if_not_null(state_ops, add_network_namespace_to_state)

    print("allow_network_namespace 1: true")
}

# This rule is when there's exactly one network namespace in the input data.
allow_network_namespace(state_ops, network_ns) := {"ops": ops, "allowed": true} {
    count(network_ns) == 1

    add_network_namespace_to_state := state_allows("network_namespace", network_ns[0].Path)
    ops := concat_op_if_not_null(state_ops, add_network_namespace_to_state)

    print("allow_network_namespace 2: true")
}

allow_masked_paths(p_oci, i_oci) {
    p_paths := p_oci.Linux.MaskedPaths
    print("allow_masked_paths 1: p_paths =", p_paths)

    i_paths := i_oci.Linux.MaskedPaths
    print("allow_masked_paths 1: i_paths =", i_paths)

    allow_masked_paths_array(p_paths, i_paths)

    print("allow_masked_paths 1: true")
}
allow_masked_paths(p_oci, i_oci) {
    print("allow_masked_paths 2: start")

    not p_oci.Linux.MaskedPaths
    not i_oci.Linux.MaskedPaths

    print("allow_masked_paths 2: true")
}

# All the policy masked paths must be masked in the input data too.
# Input is allowed to have more masked paths than the policy.
allow_masked_paths_array(p_array, i_array) {
    every p_elem in p_array {
        allow_masked_path(p_elem, i_array)
    }
}

allow_masked_path(p_elem, i_array) {
    print("allow_masked_path: p_elem =", p_elem)

    some i_elem in i_array
    p_elem == i_elem

    print("allow_masked_path: true")
}

allow_readonly_paths(p_oci, i_oci) {
    p_paths := p_oci.Linux.ReadonlyPaths
    print("allow_readonly_paths 1: p_paths =", p_paths)

    i_paths := i_oci.Linux.ReadonlyPaths
    print("allow_readonly_paths 1: i_paths =", i_paths)

    allow_readonly_paths_array(p_paths, i_paths, i_oci.Linux.MaskedPaths)

    print("allow_readonly_paths 1: true")
}
allow_readonly_paths(p_oci, i_oci) {
    print("allow_readonly_paths 2: start")

    not p_oci.Linux.ReadonlyPaths
    not i_oci.Linux.ReadonlyPaths

    print("allow_readonly_paths 2: true")
}

# All the policy readonly paths must be either:
# - Present in the input readonly paths, or
# - Present in the input masked paths.
# Input is allowed to have more readonly paths than the policy.
allow_readonly_paths_array(p_array, i_array, masked_paths) {
    every p_elem in p_array {
        allow_readonly_path(p_elem, i_array, masked_paths)
    }
}

allow_readonly_path(p_elem, i_array, masked_paths) {
    print("allow_readonly_path 1: p_elem =", p_elem)

    some i_elem in i_array
    p_elem == i_elem

    print("allow_readonly_path 1: true")
}
allow_readonly_path(p_elem, i_array, masked_paths) {
    print("allow_readonly_path 2: p_elem =", p_elem)

    some i_masked in masked_paths
    p_elem == i_masked

    print("allow_readonly_path 2: true")
}

allow_linux_devices(p_devices, i_devices) {
    print("allow_linux_devices: start")
    every i_device in i_devices {
        print("allow_linux_devices: i_device =", i_device)
        some p_device in p_devices
        i_device.Path == p_device.Path
    }
    print("allow_linux_devices: true")
}

# Check the consistency of the input "io.katacontainers.pkg.oci.bundle_path"
# and io.kubernetes.cri.sandbox-id" values with other fields.
allow_by_bundle_or_sandbox_id(p_oci, i_oci, p_storages, i_storages) {
    print("allow_by_bundle_or_sandbox_id: start")

    bundle_path := i_oci.Annotations["io.katacontainers.pkg.oci.bundle_path"]
    bundle_id := replace(bundle_path, "/run/containerd/io.containerd.runtime.v2.task/k8s.io/", "")

    key := "io.kubernetes.cri.sandbox-id"

    p_regex := p_oci.Annotations[key]
    sandbox_id := i_oci.Annotations[key]

    print("allow_by_bundle_or_sandbox_id: sandbox_id =", sandbox_id, "regex =", p_regex)
    regex.match(p_regex, sandbox_id)

    allow_root_path(p_oci, i_oci, bundle_id)

    every i_mount in input.OCI.Mounts {
        allow_mount(p_oci, i_mount, bundle_id, sandbox_id)
    }

    # TODO: enable allow_storages() after fixing https://github.com/kata-containers/kata-containers/issues/8833
    # allow_storages(p_storages, i_storages, bundle_id, sandbox_id)

    print("allow_by_bundle_or_sandbox_id: true")
}

allow_process(p_oci, i_oci, s_name) {
    p_process := p_oci.Process
    i_process := i_oci.Process

    print("allow_process: i terminal =", i_process.Terminal, "p terminal =", p_process.Terminal)
    p_process.Terminal == i_process.Terminal

    print("allow_process: i cwd =", i_process.Cwd, "i cwd =", p_process.Cwd)
    p_process.Cwd == i_process.Cwd

    print("allow_process: i noNewPrivileges =", i_process.NoNewPrivileges, "p noNewPrivileges =", p_process.NoNewPrivileges)
    p_process.NoNewPrivileges == i_process.NoNewPrivileges

    allow_caps(p_process.Capabilities, i_process.Capabilities)
    allow_user(p_process, i_process)
    allow_args(p_process, i_process, s_name)
    allow_env(p_process, i_process, s_name)

    print("allow_process: true")
}

allow_user(p_process, i_process) {
    p_user := p_process.User
    i_user := i_process.User

    print("allow_user: input uid =", i_user.UID, "policy uid =", p_user.UID)
    p_user.UID == i_user.UID

    # TODO: track down the reason for registry.k8s.io/pause:3.9 being
    #       executed with gid = 0 despite having "65535:65535" in its container image
    #       config.
    #print("allow_user: input gid =", i_user.GID, "policy gid =", p_user.GID)
    #p_user.GID == i_user.GID

    # TODO: compare the additionalGids field too after computing its value
    # based on /etc/passwd and /etc/group from the container image.
}

allow_args(p_process, i_process, s_name) {
    print("allow_args 1: no args")

    not p_process.Args
    not i_process.Args

    print("allow_args 1: true")
}
allow_args(p_process, i_process, s_name) {
    print("allow_args 2: policy args =", p_process.Args)
    print("allow_args 2: input args =", i_process.Args)

    count(p_process.Args) == count(i_process.Args)

    every i, i_arg in i_process.Args {
        allow_arg(i, i_arg, p_process, s_name)
    }

    print("allow_args 2: true")
}
allow_arg(i, i_arg, p_process, s_name) {
    p_arg := p_process.Args[i]
    print("allow_arg 1: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    p_arg2 := replace(p_arg, "$$", "$")
    p_arg2 == i_arg

    print("allow_arg 1: true")
}
allow_arg(i, i_arg, p_process, s_name) {
    p_arg := p_process.Args[i]
    print("allow_arg 2: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    # TODO: can $(node-name) be handled better?
    contains(p_arg, "$(node-name)")

    print("allow_arg 2: true")
}
allow_arg(i, i_arg, p_process, s_name) {
    p_arg := p_process.Args[i]
    print("allow_arg 3: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    p_arg2 := replace(p_arg, "$$", "$")
    p_arg3 := replace(p_arg2, "$(sandbox-name)", s_name)
    print("allow_arg 3: p_arg3 =", p_arg3)
    p_arg3 == i_arg

    print("allow_arg 3: true")
}

# OCI process.Env field
allow_env(p_process, i_process, s_name) {
    print("allow_env: p env =", p_process.Env)
    print("allow_env: i env =", i_process.Env)

    every i_var in i_process.Env {
        print("allow_env: i_var =", i_var)
        allow_var(p_process, i_process, i_var, s_name)
    }

    print("allow_env: true")
}

# Allow input env variables that are present in the policy data too.
allow_var(p_process, i_process, i_var, s_name) {
    some p_var in p_process.Env
    p_var == i_var
    print("allow_var 1: true")
}

# Match input with one of the policy variables, after substituting $(sandbox-name).
allow_var(p_process, i_process, i_var, s_name) {
    some p_var in p_process.Env
    p_var2 := replace(p_var, "$(sandbox-name)", s_name)

    print("allow_var 2: p_var2 =", p_var2)
    p_var2 == i_var

    print("allow_var 2: true")
}

# Allow input env variables that match with a request_defaults regex.
allow_var(p_process, i_process, i_var, s_name) {
    some p_regex1 in policy_data.request_defaults.CreateContainerRequest.allow_env_regex
    p_regex2 := replace(p_regex1, "$(ipv4_a)", policy_data.common.ipv4_a)
    p_regex3 := replace(p_regex2, "$(ip_p)", policy_data.common.ip_p)
    p_regex4 := replace(p_regex3, "$(svc_name)", policy_data.common.svc_name)
    p_regex5 := replace(p_regex4, "$(dns_label)", policy_data.common.dns_label)

    print("allow_var 3: p_regex5 =", p_regex5)
    regex.match(p_regex5, i_var)

    print("allow_var 3: true")
}

# Allow fieldRef "fieldPath: status.podIP" values.
allow_var(p_process, i_process, i_var, s_name) {
    name_value := split(i_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some p_var in p_process.Env
    allow_pod_ip_var(name_value[0], p_var)

    print("allow_var 4: true")
}

# Allow common fieldRef variables.
allow_var(p_process, i_process, i_var, s_name) {
    name_value := split(i_var, "=")
    count(name_value) == 2

    some p_var in p_process.Env
    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == name_value[0]

    # TODO: should these be handled in a different way?
    always_allowed := ["$(host-name)", "$(node-name)", "$(pod-uid)"]
    some allowed in always_allowed
    contains(p_name_value[1], allowed)

    print("allow_var 5: true")
}

# Allow fieldRef "fieldPath: status.hostIP" values.
allow_var(p_process, i_process, i_var, s_name) {
    name_value := split(i_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some p_var in p_process.Env
    allow_host_ip_var(name_value[0], p_var)

    print("allow_var 6: true")
}

# Allow resourceFieldRef values (e.g., "limits.cpu").
allow_var(p_process, i_process, i_var, s_name) {
    name_value := split(i_var, "=")
    count(name_value) == 2

    some p_var in p_process.Env
    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == name_value[0]

    # TODO: should these be handled in a different way?
    always_allowed = ["$(resource-field)", "$(todo-annotation)"]
    some allowed in always_allowed
    contains(p_name_value[1], allowed)

    print("allow_var 7: true")
}

allow_pod_ip_var(var_name, p_var) {
    print("allow_pod_ip_var: var_name =", var_name, "p_var =", p_var)

    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == var_name
    p_name_value[1] == "$(pod-ip)"

    print("allow_pod_ip_var: true")
}

allow_host_ip_var(var_name, p_var) {
    print("allow_host_ip_var: var_name =", var_name, "p_var =", p_var)

    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == var_name
    p_name_value[1] == "$(host-ip)"

    print("allow_host_ip_var: true")
}

is_ip(value) {
    bytes = split(value, ".")
    count(bytes) == 4

    is_ip_first_byte(bytes[0])
    is_ip_other_byte(bytes[1])
    is_ip_other_byte(bytes[2])
    is_ip_other_byte(bytes[3])
}
is_ip_first_byte(component) {
    number = to_number(component)
    number >= 1
    number <= 255
}
is_ip_other_byte(component) {
    number = to_number(component)
    number >= 0
    number <= 255
}

# OCI root.Path
allow_root_path(p_oci, i_oci, bundle_id) {
    i_path := i_oci.Root.Path
    p_path1 := p_oci.Root.Path
    print("allow_root_path: i_path =", i_path, "p_path1 =", p_path1)

    p_path2 := replace(p_path1, "$(cpath)", policy_data.common.cpath)
    print("allow_root_path: p_path2 =", p_path2)

    p_path3 := replace(p_path2, "$(bundle-id)", bundle_id)
    print("allow_root_path: p_path3 =", p_path3)

    p_path3 == i_path

    print("allow_root_path: true")
}

# device mounts
allow_mount(p_oci, i_mount, bundle_id, sandbox_id) {
    print("allow_mount: i_mount =", i_mount)

    some p_mount in p_oci.Mounts
    print("allow_mount: p_mount =", p_mount)
    check_mount(p_mount, i_mount, bundle_id, sandbox_id)

    # TODO: are there any other required policy checks for mounts - e.g.,
    #       multiple mounts with same source or destination?

    print("allow_mount: true")
}

check_mount(p_mount, i_mount, bundle_id, sandbox_id) {
    p_mount == i_mount
    print("check_mount 1: true")
}
check_mount(p_mount, i_mount, bundle_id, sandbox_id) {
    p_mount.destination == i_mount.destination
    p_mount.type_ == i_mount.type_
    p_mount.options == i_mount.options

    mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id)

    print("check_mount 2: true")
}

mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id) {
    regex1 := p_mount.source
    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    regex3 := replace(regex2, "$(cpath)", policy_data.common.mount_source_cpath)
    regex4 := replace(regex3, "$(bundle-id)", bundle_id)

    print("mount_source_allows 1: regex4 =", regex4)
    regex.match(regex4, i_mount.source)

    print("mount_source_allows 1: true")
}
mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id) {
    regex1 := p_mount.source
    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    regex3 := replace(regex2, "$(cpath)", policy_data.common.mount_source_cpath)
    regex4 := replace(regex3, "$(sandbox-id)", sandbox_id)

    print("mount_source_allows 2: regex4 =", regex4)
    regex.match(regex4, i_mount.source)

    print("mount_source_allows 2: true")
}

######################################################################
# Create container Storages

allow_storages(p_storages, i_storages, bundle_id, sandbox_id) {
    p_count := count(p_storages)
    i_count := count(i_storages)
    print("allow_storages: p_count =", p_count, "i_count =", i_count)

    p_count == i_count

    # Get the container image layer IDs and verity root hashes, from the "overlayfs" storage.
    some overlay_storage in p_storages
    overlay_storage.driver == "overlayfs"
    print("allow_storages: overlay_storage =", overlay_storage)
    count(overlay_storage.options) == 2

    layer_ids := split(overlay_storage.options[0], ":")
    print("allow_storages: layer_ids =", layer_ids)

    root_hashes := split(overlay_storage.options[1], ":")
    print("allow_storages: root_hashes =", root_hashes)

    every i_storage in i_storages {
        allow_storage(p_storages, i_storage, bundle_id, sandbox_id, layer_ids, root_hashes)
    }

    print("allow_storages: true")
}

allow_storage(p_storages, i_storage, bundle_id, sandbox_id, layer_ids, root_hashes) {
    some p_storage in p_storages

    print("allow_storage: p_storage =", p_storage)
    print("allow_storage: i_storage =", i_storage)

    p_storage.driver           == i_storage.driver
    p_storage.driver_options   == i_storage.driver_options
    p_storage.fs_group         == i_storage.fs_group

    allow_storage_options(p_storage, i_storage, layer_ids, root_hashes)
    allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids)

    # TODO: validate the source field too.

    print("allow_storage: true")
}

allow_storage_options(p_storage, i_storage, layer_ids, root_hashes) {
    print("allow_storage_options 1: start")

    p_storage.driver != "blk"
    p_storage.driver != "overlayfs"
    p_storage.options == i_storage.options

    print("allow_storage_options 1: true")
}
allow_storage_options(p_storage, i_storage, layer_ids, root_hashes) {
    print("allow_storage_options 2: start")

    p_storage.driver == "overlayfs"
    count(p_storage.options) == 2

    policy_ids := split(p_storage.options[0], ":")
    print("allow_storage_options 2: policy_ids =", policy_ids)
    policy_ids == layer_ids

    policy_hashes := split(p_storage.options[1], ":")
    print("allow_storage_options 2: policy_hashes =", policy_hashes)

    p_count := count(policy_ids)
    print("allow_storage_options 2: p_count =", p_count)
    p_count >= 1
    p_count == count(policy_hashes)

    i_count := count(i_storage.options)
    print("allow_storage_options 2: i_count =", i_count)
    i_count == p_count + 3

    print("allow_storage_options 2: i_storage.options[0] =", i_storage.options[0])
    i_storage.options[0] == "io.katacontainers.fs-opt.layer-src-prefix=/var/lib/containerd/io.containerd.snapshotter.v1.tardev/layers"

    print("allow_storage_options 2: i_storage.options[i_count - 2] =", i_storage.options[i_count - 2])
    i_storage.options[i_count - 2] == "io.katacontainers.fs-opt.overlay-rw"

    lowerdir := concat("=", ["lowerdir", p_storage.options[0]])
    print("allow_storage_options 2: lowerdir =", lowerdir)

    i_storage.options[i_count - 1] == lowerdir
    print("allow_storage_options 2: i_storage.options[i_count - 1] =", i_storage.options[i_count - 1])

    every i, policy_id in policy_ids {
        allow_overlay_layer(policy_id, policy_hashes[i], i_storage.options[i + 1])
    }

    print("allow_storage_options 2: true")
}
allow_storage_options(p_storage, i_storage, layer_ids, root_hashes) {
    print("allow_storage_options 3: start")

    p_storage.driver == "blk"
    count(p_storage.options) == 1

    startswith(p_storage.options[0], "$(hash")
    hash_suffix := trim_left(p_storage.options[0], "$(hash")

    endswith(hash_suffix, ")")
    hash_index := trim_right(hash_suffix, ")")
    i := to_number(hash_index)
    print("allow_storage_options 3: i =", i)

    hash_option := concat("=", ["io.katacontainers.fs-opt.root-hash", root_hashes[i]])
    print("allow_storage_options 3: hash_option =", hash_option)

    count(i_storage.options) == 4
    i_storage.options[0] == "ro"
    i_storage.options[1] == "io.katacontainers.fs-opt.block_device=file"
    i_storage.options[2] == "io.katacontainers.fs-opt.is-layer"
    i_storage.options[3] == hash_option

    print("allow_storage_options 3: true")
}

allow_overlay_layer(policy_id, policy_hash, i_option) {
    print("allow_overlay_layer: policy_id =", policy_id, "policy_hash =", policy_hash)
    print("allow_overlay_layer: i_option =", i_option)

    startswith(i_option, "io.katacontainers.fs-opt.layer=")
    i_value := replace(i_option, "io.katacontainers.fs-opt.layer=", "")
    i_value_decoded := base64.decode(i_value)
    print("allow_overlay_layer: i_value_decoded =", i_value_decoded)

    policy_suffix := concat("=", ["tar,ro,io.katacontainers.fs-opt.block_device=file,io.katacontainers.fs-opt.is-layer,io.katacontainers.fs-opt.root-hash", policy_hash])
    p_value := concat(",", [policy_id, policy_suffix])
    print("allow_overlay_layer: p_value =", p_value)

    p_value == i_value_decoded

    print("allow_overlay_layer: true")
}

allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids) {
    p_storage.fstype == "tar"

    startswith(p_storage.mount_point, "$(layer")
    mount_suffix := trim_left(p_storage.mount_point, "$(layer")

    endswith(mount_suffix, ")")
    layer_index := trim_right(mount_suffix, ")")
    i := to_number(layer_index)
    print("allow_mount_point 1: i =", i)

    layer_id := layer_ids[i]
    print("allow_mount_point 1: layer_id =", layer_id)

    p_mount := concat("/", ["/run/kata-containers/sandbox/layers", layer_id])
    print("allow_mount_point 1: p_mount =", p_mount)

    p_mount == i_storage.mount_point

    print("allow_mount_point 1: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids) {
    p_storage.fstype == "fuse3.kata-overlay"

    mount1 := replace(p_storage.mount_point, "$(cpath)", policy_data.common.cpath)
    mount2 := replace(mount1, "$(bundle-id)", bundle_id)
    print("allow_mount_point 2: mount2 =", mount2)

    mount2 == i_storage.mount_point

    print("allow_mount_point 2: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids) {
    p_storage.fstype == "local"

    mount1 := p_storage.mount_point
    print("allow_mount_point 3: mount1 =", mount1)

    mount2 := replace(mount1, "$(cpath)", policy_data.common.cpath)
    print("allow_mount_point 3: mount2 =", mount2)

    mount3 := replace(mount2, "$(sandbox-id)", sandbox_id)
    print("allow_mount_point 3: mount3 =", mount3)

    regex.match(mount3, i_storage.mount_point)

    print("allow_mount_point 3: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids) {
    p_storage.fstype == "bind"

    mount1 := p_storage.mount_point
    print("allow_mount_point 4: mount1 =", mount1)

    mount2 := replace(mount1, "$(cpath)", policy_data.common.cpath)
    print("allow_mount_point 4: mount2 =", mount2)

    mount3 := replace(mount2, "$(bundle-id)", bundle_id)
    print("allow_mount_point 4: mount3 =", mount3)

    regex.match(mount3, i_storage.mount_point)

    print("allow_mount_point 4: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id, layer_ids) {
    p_storage.fstype == "tmpfs"

    mount1 := p_storage.mount_point
    print("allow_mount_point 5: mount1 =", mount1)

    regex.match(mount1, i_storage.mount_point)

    print("allow_mount_point 5: true")
}

# process.Capabilities
allow_caps(p_caps, i_caps) {
    print("allow_caps: policy Ambient =", p_caps.Ambient)
    print("allow_caps: input Ambient =", i_caps.Ambient)
    match_caps(p_caps.Ambient, i_caps.Ambient)

    print("allow_caps: policy Bounding =", p_caps.Bounding)
    print("allow_caps: input Bounding =", i_caps.Bounding)
    match_caps(p_caps.Bounding, i_caps.Bounding)

    print("allow_caps: policy Effective =", p_caps.Effective)
    print("allow_caps: input Effective =", i_caps.Effective)
    match_caps(p_caps.Effective, i_caps.Effective)

    print("allow_caps: policy Inheritable =", p_caps.Inheritable)
    print("allow_caps: input Inheritable =", i_caps.Inheritable)
    match_caps(p_caps.Inheritable, i_caps.Inheritable)

    print("allow_caps: policy Permitted =", p_caps.Permitted)
    print("allow_caps: input Permitted =", i_caps.Permitted)
    match_caps(p_caps.Permitted, i_caps.Permitted)
}

match_caps(p_caps, i_caps) {
    print("match_caps 1: start")

    p_caps == i_caps

    print("match_caps 1: true")
}
match_caps(p_caps, i_caps) {
    print("match_caps 2: start")

    count(p_caps) == 1
    p_caps[0] == "$(default_caps)"

    print("match_caps 2: default_caps =", policy_data.common.default_caps)
    policy_data.common.default_caps == i_caps

    print("match_caps 2: true")
}
match_caps(p_caps, i_caps) {
    print("match_caps 3: start")

    count(p_caps) == 1
    p_caps[0] == "$(privileged_caps)"

    print("match_caps 3: privileged_caps =", policy_data.common.privileged_caps)
    policy_data.common.privileged_caps == i_caps

    print("match_caps 3: true")
}

######################################################################

check_directory_traversal(i_path) {
    not regex.match("(^|/)..($|/)", i_path)
}

allow_sandbox_storages(i_storages) {
    print("allow_sandbox_storages: i_storages =", i_storages)

    p_storages := policy_data.sandbox.storages
    every i_storage in i_storages {
        allow_sandbox_storage(p_storages, i_storage)
    }

    print("allow_sandbox_storages: true")
}

allow_sandbox_storage(p_storages, i_storage) {
    print("allow_sandbox_storage: i_storage =", i_storage)

    some p_storage in p_storages
    print("allow_sandbox_storage: p_storage =", p_storage)
    i_storage == p_storage

    print("allow_sandbox_storage: true")
}

CopyFileRequest {
    print("CopyFileRequest: input.path =", input.path)

    check_directory_traversal(input.path)

    some regex1 in policy_data.request_defaults.CopyFileRequest
    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    regex3 := replace(regex2, "$(cpath)", policy_data.common.mount_source_cpath)
    regex4 := replace(regex3, "$(bundle-id)", "[a-z0-9]{64}")
    print("CopyFileRequest: regex4 =", regex4)

    regex.match(regex4, input.path)

    print("CopyFileRequest: true")
}

CreateSandboxRequest {
    print("CreateSandboxRequest: input.guest_hook_path =", input.guest_hook_path)
    count(input.guest_hook_path) == 0

    print("CreateSandboxRequest: input.kernel_modules =", input.kernel_modules)
    count(input.kernel_modules) == 0

    i_pidns := input.sandbox_pidns
    print("CreateSandboxRequest: i_pidns =", i_pidns)
    i_pidns == false
    allow_sandbox_storages(input.storages)
}

ExecProcessRequest {
    print("ExecProcessRequest 1: input =", input)

    some p_command in policy_data.request_defaults.ExecProcessRequest.allowed_commands
    print("ExecProcessRequest 1: p_command =", p_command)
    p_command == input.process.Args

    print("ExecProcessRequest 1: true")
}
ExecProcessRequest {
    print("ExecProcessRequest 2: input =", input)

    # TODO: match input container ID with its corresponding container.exec_commands.
    some container in policy_data.containers
    some p_command in container.exec_commands
    print("ExecProcessRequest 2: p_command =", p_command)

    # TODO: should other input data fields be validated as well?
    p_command == input.process.Args

    print("ExecProcessRequest 2: true")
}
ExecProcessRequest {
    print("ExecProcessRequest 3: input =", input)

    i_command = concat(" ", input.process.Args)
    print("ExecProcessRequest 3: i_command =", i_command)

    some p_regex in policy_data.request_defaults.ExecProcessRequest.regex
    print("ExecProcessRequest 3: p_regex =", p_regex)

    regex.match(p_regex, i_command)

    print("ExecProcessRequest 3: true")
}

CloseStdinRequest {
    policy_data.request_defaults.CloseStdinRequest == true
}

ReadStreamRequest {
    policy_data.request_defaults.ReadStreamRequest == true
}

UpdateEphemeralMountsRequest {
    policy_data.request_defaults.UpdateEphemeralMountsRequest == true
}

WriteStreamRequest {
    policy_data.request_defaults.WriteStreamRequest == true
}
