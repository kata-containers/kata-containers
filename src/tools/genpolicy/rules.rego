# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
package agent_policy

# Not required for rego v1. regorus still defaults to v0, so keep it for now.
import future.keywords.in
import future.keywords.every
import future.keywords.if

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
default UpdateInterfaceRequest := false
default UpdateRoutesRequest := false
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

CreateContainerRequest := {"ops": ops, "allowed": true} if {
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
    some idx, p_container in policy_data.containers
    print("======== CreateContainerRequest: trying next policy container")

    p_pidns := p_container.sandbox_pidns
    i_pidns := input.sandbox_pidns
    print("CreateContainerRequest: p_pidns =", p_pidns, "i_pidns =", i_pidns)
    p_pidns == i_pidns

    p_oci := p_container.OCI

    # check namespace
    p_namespace := p_oci.Annotations[S_NAMESPACE_KEY]
    i_namespace := i_oci.Annotations[S_NAMESPACE_KEY]
    print("CreateContainerRequest: p_namespace =", p_namespace, "i_namespace =", i_namespace)
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

    # save to policy state
    # key: input.container_id
    # val: index of p_container in the policy_data.containers array
    print("CreateContainerRequest: addding container_id=", input.container_id, " to state")
    add_p_container_to_state := state_allows(input.container_id, idx)

    ops := concat_op_if_not_null(ret.ops, add_p_container_to_state)

    print("CreateContainerRequest: true")
}

allow_create_container_input if {
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

    i_process := i_oci.Process
    count(i_process.SelinuxLabel) == 0
    count(i_process.User.Username) == 0

    print("allow_create_container_input: true")
}

allow_namespace(p_namespace, i_namespace) = add_namespace if {
    p_namespace == i_namespace
    add_namespace := state_allows("namespace", i_namespace)
    print("allow_namespace 1: input namespace matches policy data")
}

allow_namespace(p_namespace, i_namespace) = add_namespace if {
    p_namespace == ""
    print("allow_namespace 2: no namespace found on policy data")
    add_namespace := state_allows("namespace", i_namespace)
}

# key hasn't been seen before, save key, value pair to state
state_allows(key, value) = action if {
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
state_allows(key, value) = action if {
  print("state_allows 2: start")
  state := get_state()
  print("state_allows 2: state[key] =", state[key], "value =", value)
  value == state[key]
  print("state_allows 2: found key =", key, "value =", value, " in state")
  action := null
}

# delete key=value from state
state_del_key(key) = action if {
  print("state_del_key: ", key)
  state := get_state()
  print("state_del_key: deleting from state key =", key)
  path := get_state_path(key)
  action := {
    "op": "remove",
    "path": path,
  }
}

# helper functions to interact with the state
get_state() = state if {
  state := data["pstate"]
}

get_state_val(key) = value if {
    state := get_state()
    value := state[key]
}

get_state_path(key) = path if {
    # prepend "/pstate/" to key
    path := concat("/", ["/pstate", key])
}

# Helper functions to conditionally concatenate op is not null
concat_op_if_not_null(ops, op) = result if {
    op == null
    result := ops
}

concat_op_if_not_null(ops, op) = result if {
    op != null
    result := array.concat(ops, [op])
}

# Reject unexpected annotations.
allow_anno(p_oci, i_oci) if {
    print("allow_anno 1: start")

    not i_oci.Annotations

    print("allow_anno 1: true")
}
allow_anno(p_oci, i_oci) if {
    print("allow_anno 2: p Annotations =", p_oci.Annotations)
    print("allow_anno 2: i Annotations =", i_oci.Annotations)

    i_keys := object.keys(i_oci.Annotations)
    print("allow_anno 2: i keys =", i_keys)

    every i_key in i_keys {
        allow_anno_key(i_key, p_oci)
    }

    print("allow_anno 2: true")
}

allow_anno_key(i_key, p_oci) if {
    print("allow_anno_key 1: i key =", i_key)

    startswith(i_key, "io.kubernetes.cri.")

    print("allow_anno_key 1: true")
}
allow_anno_key(i_key, p_oci) if {
    print("allow_anno_key 2: i key =", i_key)

    some p_key, _ in p_oci.Annotations
    p_key == i_key

    print("allow_anno_key 2: true")
}

# Get the value of the S_NAME_KEY annotation and
# correlate it with other annotations and process fields.
allow_by_anno(p_oci, i_oci, p_storages, i_storages) if {
    print("allow_by_anno 1: start")

    not p_oci.Annotations[S_NAME_KEY]

    i_s_name := i_oci.Annotations[S_NAME_KEY]
    print("allow_by_anno 1: i_s_name =", i_s_name)

    i_s_namespace := i_oci.Annotations[S_NAMESPACE_KEY]
    print("allow_by_anno 1: i_s_namespace =", i_s_namespace)

    allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, i_s_name, i_s_namespace)

    print("allow_by_anno 1: true")
}
allow_by_anno(p_oci, i_oci, p_storages, i_storages) if {
    print("allow_by_anno 2: start")

    p_s_name := p_oci.Annotations[S_NAME_KEY]
    i_s_name := i_oci.Annotations[S_NAME_KEY]
    print("allow_by_anno 2: i_s_name =", i_s_name, "p_s_name =", p_s_name)

    allow_sandbox_name(p_s_name, i_s_name)

    i_s_namespace := i_oci.Annotations[S_NAMESPACE_KEY]
    print("allow_by_anno 2: i_s_namespace =", i_s_namespace)

    allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, i_s_name, i_s_namespace)

    print("allow_by_anno 2: true")
}

allow_by_sandbox_name(p_oci, i_oci, p_storages, i_storages, s_name, s_namespace) if {
    print("allow_by_sandbox_name: start")

    i_namespace := i_oci.Annotations[S_NAMESPACE_KEY]

    allow_by_container_types(p_oci, i_oci, s_name, i_namespace)
    allow_by_bundle_or_sandbox_id(p_oci, i_oci, p_storages, i_storages)
    allow_process(p_oci.Process, i_oci.Process, s_name, s_namespace)

    print("allow_by_sandbox_name: true")
}

allow_sandbox_name(p_s_name, i_s_name) if {
    print("allow_sandbox_name: start")
    regex.match(p_s_name, i_s_name)

    print("allow_sandbox_name: true")
}

# Check that the "io.kubernetes.cri.container-type" and
# "io.katacontainers.pkg.oci.container_type" annotations designate the
# expected type - either a "sandbox" or a "container". Then, validate
# other annotations based on the actual "sandbox" or "container" value
# from the input container.
allow_by_container_types(p_oci, i_oci, s_name, s_namespace) if {
    print("allow_by_container_types: checking io.kubernetes.cri.container-type")

    c_type := "io.kubernetes.cri.container-type"

    p_cri_type := p_oci.Annotations[c_type]
    i_cri_type := i_oci.Annotations[c_type]
    print("allow_by_container_types: p_cri_type =", p_cri_type, "i_cri_type =", i_cri_type)
    p_cri_type == i_cri_type

    allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace)

    print("allow_by_container_types: true")
}

allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace) if {
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

allow_by_container_type(i_cri_type, p_oci, i_oci, s_name, s_namespace) if {
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
allow_sandbox_container_name(p_oci, i_oci) if {
    print("allow_sandbox_container_name: start")

    container_annotation_missing(p_oci, i_oci, "io.kubernetes.cri.container-name")

    print("allow_sandbox_container_name: true")
}

allow_container_name(p_oci, i_oci) if {
    print("allow_container_name: start")

    allow_container_annotation(p_oci, i_oci, "io.kubernetes.cri.container-name")

    print("allow_container_name: true")
}

container_annotation_missing(p_oci, i_oci, key) if {
    print("container_annotation_missing:", key)

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("container_annotation_missing: true")
}

allow_container_annotation(p_oci, i_oci, key) if {
    print("allow_container_annotation: key =", key)

    p_value := p_oci.Annotations[key]
    i_value := i_oci.Annotations[key]
    print("allow_container_annotation: p_value =", p_value, "i_value =", i_value)

    p_value == i_value

    print("allow_container_annotation: true")
}

# "nerdctl/network-namespace" annotation
allow_sandbox_net_namespace(p_oci, i_oci) if {
    print("allow_sandbox_net_namespace: start")

    key := "nerdctl/network-namespace"

    p_namespace := p_oci.Annotations[key]
    i_namespace := i_oci.Annotations[key]
    print("allow_sandbox_net_namespace: p_namespace =", p_namespace, "i_namespace =", i_namespace)

    regex.match(p_namespace, i_namespace)

    print("allow_sandbox_net_namespace: true")
}

allow_net_namespace(p_oci, i_oci) if {
    print("allow_net_namespace: start")

    key := "nerdctl/network-namespace"

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("allow_net_namespace: true")
}

# "io.kubernetes.cri.sandbox-log-directory" annotation
allow_sandbox_log_directory(p_oci, i_oci, s_name, s_namespace) if {
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

allow_log_directory(p_oci, i_oci) if {
    print("allow_log_directory: start")

    key := "io.kubernetes.cri.sandbox-log-directory"

    not p_oci.Annotations[key]
    not i_oci.Annotations[key]

    print("allow_log_directory: true")
}

allow_devices(p_devices, i_devices) if {
    print("allow_devices: start")
    every i_device in i_devices {
        print("allow_devices: i_device =", i_device)
        some p_device in p_devices
        p_device.container_path == i_device.container_path
    }
    print("allow_devices: true")
}


allow_linux(state_ops, p_oci, i_oci) := {"ops": ops, "allowed": true} if {
    p_namespaces := p_oci.Linux.Namespaces
    print("allow_linux: p namespaces =", p_namespaces)

    p_namespaces_normalized := [
        {"Path": obj.Path, "Type": normalize_namespace_type(obj.Type)}
        | obj := p_namespaces[_]
    ]

    i_namespaces := i_oci.Linux.Namespaces
    print("allow_linux: i namespaces =", i_namespaces)

    i_namespace_without_network_normalized := [
        {"Path": obj.Path, "Type": normalize_namespace_type(obj.Type)}
        | obj := i_namespaces[_]; obj.Type != "network"; obj.Type != "cgroup"
    ]

    print("allow_linux: p_namespaces_normalized =", p_namespaces_normalized)
    print("allow_linux: i_namespace_without_network_normalized =", i_namespace_without_network_normalized)

    p_namespaces_normalized == i_namespace_without_network_normalized

    allow_masked_paths(p_oci, i_oci)
    allow_readonly_paths(p_oci, i_oci)
    allow_linux_devices(p_oci.Linux.Devices, i_oci.Linux.Devices)
    allow_linux_sysctl(p_oci.Linux, i_oci.Linux)
    ret := allow_network_namespace_start(state_ops, p_oci, i_oci)
    ret.allowed

    ops := ret.ops

    print("allow_linux: true")
}

# Retrieve the "network" namespace from the input data and pass it on for the
# network namespace policy checks.
allow_network_namespace_start(state_ops, p_oci, i_oci) := {"ops": ops, "allowed": true} if {
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
allow_network_namespace(state_ops, network_ns) := {"ops": ops, "allowed": true} if {
    count(network_ns) == 0

    network_ns_path = ""

    add_network_namespace_to_state := state_allows("network_namespace", network_ns_path)
    ops := concat_op_if_not_null(state_ops, add_network_namespace_to_state)

    print("allow_network_namespace 1: true")
}

# This rule is when there's exactly one network namespace in the input data.
allow_network_namespace(state_ops, network_ns) := {"ops": ops, "allowed": true} if {
    count(network_ns) == 1

    add_network_namespace_to_state := state_allows("network_namespace", network_ns[0].Path)
    ops := concat_op_if_not_null(state_ops, add_network_namespace_to_state)

    print("allow_network_namespace 2: true")
}

allow_masked_paths(p_oci, i_oci) if {
    p_paths := p_oci.Linux.MaskedPaths
    print("allow_masked_paths 1: p_paths =", p_paths)

    i_paths := i_oci.Linux.MaskedPaths
    print("allow_masked_paths 1: i_paths =", i_paths)

    allow_masked_paths_array(p_paths, i_paths)

    print("allow_masked_paths 1: true")
}
allow_masked_paths(p_oci, i_oci) if {
    print("allow_masked_paths 2: start")

    not p_oci.Linux.MaskedPaths
    not i_oci.Linux.MaskedPaths

    print("allow_masked_paths 2: true")
}

# All the policy masked paths must be masked in the input data too.
# Input is allowed to have more masked paths than the policy.
allow_masked_paths_array(p_array, i_array) if {
    every p_elem in p_array {
        allow_masked_path(p_elem, i_array)
    }
}

allow_masked_path(p_elem, i_array) if {
    print("allow_masked_path: p_elem =", p_elem)

    some i_elem in i_array
    p_elem == i_elem

    print("allow_masked_path: true")
}

allow_readonly_paths(p_oci, i_oci) if {
    p_paths := p_oci.Linux.ReadonlyPaths
    print("allow_readonly_paths 1: p_paths =", p_paths)

    i_paths := i_oci.Linux.ReadonlyPaths
    print("allow_readonly_paths 1: i_paths =", i_paths)

    allow_readonly_paths_array(p_paths, i_paths, i_oci.Linux.MaskedPaths)

    print("allow_readonly_paths 1: true")
}
allow_readonly_paths(p_oci, i_oci) if {
    print("allow_readonly_paths 2: start")

    not p_oci.Linux.ReadonlyPaths
    not i_oci.Linux.ReadonlyPaths

    print("allow_readonly_paths 2: true")
}

# All the policy readonly paths must be either:
# - Present in the input readonly paths, or
# - Present in the input masked paths.
# Input is allowed to have more readonly paths than the policy.
allow_readonly_paths_array(p_array, i_array, masked_paths) if {
    every p_elem in p_array {
        allow_readonly_path(p_elem, i_array, masked_paths)
    }
}

allow_readonly_path(p_elem, i_array, masked_paths) if {
    print("allow_readonly_path 1: p_elem =", p_elem)

    some i_elem in i_array
    p_elem == i_elem

    print("allow_readonly_path 1: true")
}
allow_readonly_path(p_elem, i_array, masked_paths) if {
    print("allow_readonly_path 2: p_elem =", p_elem)

    some i_masked in masked_paths
    p_elem == i_masked

    print("allow_readonly_path 2: true")
}

allow_linux_devices(p_devices, i_devices) if {
    print("allow_linux_devices: start")
    every i_device in i_devices {
        print("allow_linux_devices: i_device =", i_device)
        some p_device in p_devices
        i_device.Path == p_device.Path
    }
    print("allow_linux_devices: true")
}

allow_linux_sysctl(p_linux, i_linux) if {
    print("allow_linux_sysctl 1: start")
    not i_linux.Sysctl
    print("allow_linux_sysctl 1: true")
}

allow_linux_sysctl(p_linux, i_linux) if {
    print("allow_linux_sysctl 2: start")
    p_sysctl := p_linux.Sysctl
    i_sysctl := i_linux.Sysctl
    every i_name, i_val in i_sysctl {
        print("allow_linux_sysctl 2: i_name =", i_name, "i_val =", i_val)
        p_sysctl[i_name] == i_val
    }
    print("allow_linux_sysctl 2: true")
}

# Check the consistency of the input "io.katacontainers.pkg.oci.bundle_path"
# and io.kubernetes.cri.sandbox-id" values with other fields.
allow_by_bundle_or_sandbox_id(p_oci, i_oci, p_storages, i_storages) if {
    print("allow_by_bundle_or_sandbox_id: start")

    bundle_path := i_oci.Annotations["io.katacontainers.pkg.oci.bundle_path"]
    bundle_id := replace(bundle_path, "/run/containerd/io.containerd.runtime.v2.task/k8s.io/", "")

    key := "io.kubernetes.cri.sandbox-id"

    p_regex := p_oci.Annotations[key]
    sandbox_id := i_oci.Annotations[key]

    print("allow_by_bundle_or_sandbox_id: sandbox_id =", sandbox_id, "regex =", p_regex)
    regex.match(p_regex, sandbox_id)

    allow_root_path(p_oci, i_oci, bundle_id)

    # Match each input mount with a Policy mount.
    # Reject possible attempts to match multiple input mounts with a single Policy mount.
    p_matches := { p_index | some i_index; p_index = allow_mount(p_oci, input.OCI.Mounts[i_index], bundle_id, sandbox_id) }

    print("allow_by_bundle_or_sandbox_id: p_matches =", p_matches)
    count(p_matches) == count(input.OCI.Mounts)

    allow_storages(p_storages, i_storages, bundle_id, sandbox_id)

    print("allow_by_bundle_or_sandbox_id: true")
}

allow_process_common(p_process, i_process, s_name, s_namespace) if {
    print("allow_process_common: p_process =", p_process)
    print("allow_process_common: i_process = ", i_process)
    print("allow_process_common: s_name =", s_name)

    p_process.Cwd == i_process.Cwd
    p_process.NoNewPrivileges == i_process.NoNewPrivileges

    allow_user(p_process, i_process)
    allow_env(p_process, i_process, s_name, s_namespace)

    print("allow_process_common: true")
}

# Compare the OCI Process field of a policy container with the input OCI Process from a CreateContainerRequest
allow_process(p_process, i_process, s_name, s_namespace) if {
    print("allow_process: start")

    allow_args(p_process, i_process, s_name)
    allow_process_common(p_process, i_process, s_name, s_namespace)
    allow_caps(p_process.Capabilities, i_process.Capabilities)
    p_process.Terminal == i_process.Terminal

    print("allow_process: true")
}

# Compare the OCI Process field of a policy container with the input process field from ExecProcessRequest
allow_interactive_process(p_process, i_process, s_name, s_namespace) if {
    print("allow_interactive_process: start")

    allow_process_common(p_process, i_process, s_name, s_namespace)
    allow_exec_caps(i_process.Capabilities)

    # These are commands enabled using ExecProcessRequest commands and/or regex from the settings file.
    # They can be executed interactively so allow them to use any value for i_process.Terminal.

    print("allow_interactive_process: true")
}

# Compare the OCI Process field of a policy container with the input process field from ExecProcessRequest
allow_probe_process(p_process, i_process, s_name, s_namespace) if {
    print("allow_probe_process: start")

    allow_process_common(p_process, i_process, s_name, s_namespace)
    allow_exec_caps(i_process.Capabilities)
    p_process.Terminal == i_process.Terminal

    print("allow_probe_process: true")
}

allow_user(p_process, i_process) if {
    p_user := p_process.User
    i_user := i_process.User

    print("allow_user: input uid =", i_user.UID, "policy uid =", p_user.UID)
    p_user.UID == i_user.UID

    print("allow_user: input gid =", i_user.GID, "policy gid =", p_user.GID)
    p_user.GID == i_user.GID

    print("allow_user: input additionalGids =", i_user.AdditionalGids, "policy additionalGids =", p_user.AdditionalGids)
    {e | some e in p_user.AdditionalGids} == {e | some e in i_user.AdditionalGids}
}

allow_args(p_process, i_process, s_name) if {
    print("allow_args 1: no args")

    not p_process.Args
    not i_process.Args

    print("allow_args 1: true")
}
allow_args(p_process, i_process, s_name) if {
    print("allow_args 2: policy args =", p_process.Args)
    print("allow_args 2: input args =", i_process.Args)

    count(p_process.Args) == count(i_process.Args)

    every i, i_arg in i_process.Args {
        allow_arg(i, i_arg, p_process, s_name)
    }

    print("allow_args 2: true")
}
allow_arg(i, i_arg, p_process, s_name) if {
    p_arg := p_process.Args[i]
    print("allow_arg 1: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    p_arg2 := replace(p_arg, "$$", "$")
    p_arg2 == i_arg

    print("allow_arg 1: true")
}
allow_arg(i, i_arg, p_process, s_name) if {
    p_arg := p_process.Args[i]
    print("allow_arg 2: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    # TODO: can $(node-name) be handled better?
    contains(p_arg, "$(node-name)")

    print("allow_arg 2: true")
}
allow_arg(i, i_arg, p_process, s_name) if {
    p_arg := p_process.Args[i]
    print("allow_arg 3: i =", i, "i_arg =", i_arg, "p_arg =", p_arg)

    p_arg2 := replace(p_arg, "$$", "$")
    p_arg3 := replace(p_arg2, "$(sandbox-name)", s_name)
    print("allow_arg 3: p_arg3 =", p_arg3)
    p_arg3 == i_arg

    print("allow_arg 3: true")
}

# OCI process.Env field
allow_env(p_process, i_process, s_name, s_namespace) if {
    print("allow_env: p env =", p_process.Env)
    print("allow_env: i env =", i_process.Env)

    every i_var in i_process.Env {
        print("allow_env: i_var =", i_var)
        allow_var(p_process, i_process, i_var, s_name, s_namespace)
    }

    print("allow_env: true")
}

# Allow input env variables that are present in the policy data too.
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    some p_var in p_process.Env
    p_var == i_var
    print("allow_var 1: true")
}

# Match input with one of the policy variables, after substituting $(sandbox-name).
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    some p_var in p_process.Env
    p_var2 := replace(p_var, "$(sandbox-name)", s_name)

    print("allow_var 2: p_var =", p_var)

    p_var_split := split(p_var, "=")
    count(p_var_split) == 2

    p_var_split[1] == "$(sandbox-name)"

    i_var_split := split(i_var, "=")
    count(i_var_split) == 2

    i_var_split[0] == p_var_split[0]
    regex.match(s_name, i_var_split[1])

    print("allow_var 2: true")
}

# Allow input env variables that match with a request_defaults regex.
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    some p_regex1 in policy_data.request_defaults.CreateContainerRequest.allow_env_regex
    p_regex2 := replace(p_regex1, "$(ipv4_a)", policy_data.common.ipv4_a)
    p_regex3 := replace(p_regex2, "$(ip_p)", policy_data.common.ip_p)
    p_regex4 := replace(p_regex3, "$(svc_name_downward_env)", policy_data.common.svc_name_downward_env)
    p_regex5 := replace(p_regex4, "$(dns_label)", policy_data.common.dns_label)

    print("allow_var 3: p_regex5 =", p_regex5)
    regex.match(p_regex5, i_var)

    print("allow_var 3: true")
}

# Allow fieldRef "fieldPath: status.podIP" values.
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    name_value := split(i_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some p_var in p_process.Env
    allow_pod_ip_var(name_value[0], p_var)

    print("allow_var 4: true")
}

# Allow common fieldRef variables.
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
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
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    name_value := split(i_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some p_var in p_process.Env
    allow_host_ip_var(name_value[0], p_var)

    print("allow_var 6: true")
}

# Allow resourceFieldRef values (e.g., "limits.cpu").
allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
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

allow_var(p_process, i_process, i_var, s_name, s_namespace) if {
    some p_var in p_process.Env
    p_var2 := replace(p_var, "$(sandbox-namespace)", s_namespace)

    print("allow_var 8: p_var2 =", p_var2)
    p_var2 == i_var

    print("allow_var 8: true")
}

allow_pod_ip_var(var_name, p_var) if {
    print("allow_pod_ip_var: var_name =", var_name, "p_var =", p_var)

    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == var_name
    p_name_value[1] == "$(pod-ip)"

    print("allow_pod_ip_var: true")
}

allow_host_ip_var(var_name, p_var) if {
    print("allow_host_ip_var: var_name =", var_name, "p_var =", p_var)

    p_name_value := split(p_var, "=")
    count(p_name_value) == 2

    p_name_value[0] == var_name
    p_name_value[1] == "$(host-ip)"

    print("allow_host_ip_var: true")
}

is_ip(value) if {
    bytes = split(value, ".")
    count(bytes) == 4

    is_ip_first_byte(bytes[0])
    is_ip_other_byte(bytes[1])
    is_ip_other_byte(bytes[2])
    is_ip_other_byte(bytes[3])
}
is_ip_first_byte(component) if {
    number = to_number(component)
    number >= 1
    number <= 255
}
is_ip_other_byte(component) if {
    number = to_number(component)
    number >= 0
    number <= 255
}

# OCI root.Path
allow_root_path(p_oci, i_oci, bundle_id) if {
    i_path := i_oci.Root.Path
    p_path1 := p_oci.Root.Path
    print("allow_root_path: i_path =", i_path, "p_path1 =", p_path1)

    p_path2 := replace(p_path1, "$(root_path)", policy_data.common.root_path)
    print("allow_root_path: p_path2 =", p_path2)

    p_path3 := replace(p_path2, "$(bundle-id)", bundle_id)
    print("allow_root_path: p_path3 =", p_path3)

    p_path3 == i_path

    print("allow_root_path: true")
}

# device mounts
# allow_mount returns the policy index (p_index) if a given input mount matches a policy mount.
allow_mount(p_oci, i_mount, bundle_id, sandbox_id):= p_index if {
    print("allow_mount: i_mount =", i_mount)

    some p_index, p_mount in p_oci.Mounts
    print("allow_mount: p_index =", p_index, "p_mount =", p_mount)
    check_mount(p_mount, i_mount, bundle_id, sandbox_id)

    print("allow_mount: true, p_index =", p_index)
}

check_mount(p_mount, i_mount, bundle_id, sandbox_id) if {
    p_mount == i_mount
    print("check_mount 1: true")
}
check_mount(p_mount, i_mount, bundle_id, sandbox_id) if {
    p_mount.destination == i_mount.destination
    p_mount.type_ == i_mount.type_
    p_mount.options == i_mount.options

    mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id)

    print("check_mount 2: true")
}

mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id) if {
    regex1 := p_mount.source
    print("mount_source_allows 1: regex1 =", regex1)

    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    print("mount_source_allows 1: regex2 =", regex2)

    regex3 := replace(regex2, "$(cpath)", policy_data.common.cpath)
    print("mount_source_allows 1: regex3 =", regex3)

    regex4 := replace(regex3, "$(bundle-id)", bundle_id)
    print("mount_source_allows 1: regex4 =", regex4)
    regex.match(regex4, i_mount.source)

    print("mount_source_allows 1: true")
}
mount_source_allows(p_mount, i_mount, bundle_id, sandbox_id) if {
    regex1 := p_mount.source
    print("mount_source_allows 2: regex1 =", regex1)

    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    print("mount_source_allows 2: regex2 =", regex2)

    regex3 := replace(regex2, "$(cpath)", policy_data.common.cpath)
    print("mount_source_allows 2: regex3 =", regex3)

    regex4 := replace(regex3, "$(sandbox-id)", sandbox_id)
    print("mount_source_allows 2: regex4 =", regex4)
    regex.match(regex4, i_mount.source)

    print("mount_source_allows 2: true")
}

######################################################################
# Create container Storages

allow_storages(p_storages, i_storages, bundle_id, sandbox_id) if {
    print("allow_storages: p_storages =", p_storages)
    print("allow_storages: i_storages =", i_storages)

    p_count := count(p_storages)
    i_count := count(i_storages)
    img_pull_count := count([s | s := i_storages[_]; s.driver == "image_guest_pull"])
    print("allow_storages: p_count =", p_count, "i_count =", i_count, "img_pull_count =", img_pull_count)

    p_count == i_count - img_pull_count

    every i_storage in i_storages {
        allow_storage(p_storages, i_storage, bundle_id, sandbox_id)
    }

    print("allow_storages: true")
}

allow_storage(p_storages, i_storage, bundle_id, sandbox_id) if {
    some p_storage in p_storages

    print("allow_storage: p_storage =", p_storage)
    print("allow_storage: i_storage =", i_storage)

    p_storage.driver           == i_storage.driver
    p_storage.driver_options   == i_storage.driver_options
    p_storage.fs_group         == i_storage.fs_group
    p_storage.fstype           == i_storage.fstype

    allow_storage_source(p_storage, i_storage, bundle_id)
    allow_storage_options(p_storage, i_storage)
    allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id)

    print("allow_storage: true")
}
allow_storage(p_storages, i_storage, bundle_id, sandbox_id) if {
    i_storage.driver == "image_guest_pull"
    print("allow_storage with image_guest_pull: start")
    i_storage.fstype == "overlay"
    i_storage.fs_group == null
    count(i_storage.options) == 0
    # TODO: Check Mount Point, Source, Driver Options, etc.
    print("allow_storage with image_guest_pull: true")
}

allow_storage_source(p_storage, i_storage, bundle_id) if {
    print("allow_storage_source 1: start")

    p_storage.source == i_storage.source

    print("allow_storage_source 1: true")
}
allow_storage_source(p_storage, i_storage, bundle_id) if {
    print("allow_storage_source 2: start")

    source1 := p_storage.source
    source2 := replace(source1, "$(sfprefix)", policy_data.common.sfprefix)
    source3 := replace(source2, "$(cpath)", policy_data.common.cpath)
    source4 := replace(source3, "$(bundle-id)", bundle_id)
    
    print("allow_storage_source 2: source =", source4)
    regex.match(source4, i_storage.source)

    print("allow_storage_source 2: true")
}
allow_storage_source(p_storage, i_storage, bundle_id) if {
    print("allow_storage_source 3: start")

    p_storage.driver == "overlayfs"
    i_storage.source == "none"

    print("allow_storage_source 3: true")
}

allow_storage_options(p_storage, i_storage) if {
    print("allow_storage_options 1: start")

    p_storage.driver != "blk"
    p_storage.driver != "overlayfs"
    p_storage.options == i_storage.options

    print("allow_storage_options 1: true")
}

allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id) if {
    p_storage.fstype == "local"

    mount1 := p_storage.mount_point
    print("allow_mount_point 3: mount1 =", mount1)

    mount2 := replace(mount1, "$(cpath)", policy_data.common.cpath)
    print("allow_mount_point 1: mount2 =", mount2)

    mount3 := replace(mount2, "$(sandbox-id)", sandbox_id)
    print("allow_mount_point 1: mount3 =", mount3)

    regex.match(mount3, i_storage.mount_point)

    print("allow_mount_point 1: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id) if {
    p_storage.fstype == "bind"

    mount1 := p_storage.mount_point
    print("allow_mount_point 2: mount1 =", mount1)

    mount2 := replace(mount1, "$(cpath)", policy_data.common.cpath)
    print("allow_mount_point 2: mount2 =", mount2)

    mount3 := replace(mount2, "$(bundle-id)", bundle_id)
    print("allow_mount_point 2: mount3 =", mount3)

    regex.match(mount3, i_storage.mount_point)

    print("allow_mount_point 2: true")
}
allow_mount_point(p_storage, i_storage, bundle_id, sandbox_id) if {
    p_storage.fstype == "tmpfs"

    mount1 := p_storage.mount_point
    print("allow_mount_point 3: mount1 =", mount1)

    regex.match(mount1, i_storage.mount_point)

    print("allow_mount_point 3: true")
}

# ExecProcessRequest.process.Capabilities
allow_exec_caps(i_caps) if {
    not i_caps.Ambient
    not i_caps.Bounding
    not i_caps.Effective
    not i_caps.Inheritable
    not i_caps.Permitted
}

# OCI.Process.Capabilities
allow_caps(p_caps, i_caps) if {
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

match_caps(p_caps, i_caps) if {
    print("match_caps 1: start")

    norm_policy := { strip_cap_prefix(c) | c := p_caps[_] }
    norm_input := { strip_cap_prefix(c) | c := i_caps[_] }

    norm_policy == norm_input

    print("match_caps 1: true")
}
match_caps(p_caps, i_caps) if {
    print("match_caps 2: start")

    count(p_caps) == 1
    p_caps[0] == "$(default_caps)"

    print("match_caps 2: i_caps =", i_caps)
    print("match_caps 2: default_caps =", policy_data.common.default_caps)

    norm_defaults := { strip_cap_prefix(c) | c := policy_data.common.default_caps[_] }
    norm_input := { strip_cap_prefix(c) | c := i_caps[_] }
    print("match_caps 2: norm_defaults =", norm_defaults)
    print("match_caps 2: norm_input    =", norm_input)

    norm_defaults == norm_input

    print("match_caps 2: true")
}
match_caps(p_caps, i_caps) if {
    print("match_caps 3: start")

    count(p_caps) == 1
    p_caps[0] == "$(privileged_caps)"

    print("match_caps 3: i_caps =", i_caps)
    print("match_caps 3: privileged_caps =", policy_data.common.privileged_caps)

    norm_defaults := { strip_cap_prefix(c) | c := policy_data.common.privileged_caps[_] }
    norm_input    := { strip_cap_prefix(c) | c := i_caps[_] }
    print("match_caps 3: norm_defaults =", norm_defaults)
    print("match_caps 3: norm_input    =", norm_input)

    norm_defaults == norm_input

    print("match_caps 3: true")
}

######################################################################

normalize_namespace_type(type) := normalized_type if {
    lower(type) == "mount"
    normalized_type := "mnt"
} else := normalized_type if {
    normalized_type := type
}

strip_cap_prefix(s) := result if {
    startswith(s, "CAP_")
    result := substring(s, 4, count(s) - 4)
} else := result if {
    result := s
}

check_directory_traversal(i_path) if {
    not regex.match("(^|/)..($|/)", i_path)
}

allow_sandbox_storages(i_storages) if {
    print("allow_sandbox_storages: i_storages =", i_storages)

    p_storages := policy_data.sandbox.storages
    every i_storage in i_storages {
        allow_sandbox_storage(p_storages, i_storage)
    }

    print("allow_sandbox_storages: true")
}

allow_sandbox_storage(p_storages, i_storage) if {
    print("allow_sandbox_storage: i_storage =", i_storage)

    some p_storage in p_storages
    print("allow_sandbox_storage: p_storage =", p_storage)
    i_storage == p_storage

    print("allow_sandbox_storage: true")
}

CopyFileRequest if {
    print("CopyFileRequest: input.path =", input.path)

    check_directory_traversal(input.path)

    some regex1 in policy_data.request_defaults.CopyFileRequest
    regex2 := replace(regex1, "$(sfprefix)", policy_data.common.sfprefix)
    regex3 := replace(regex2, "$(cpath)", policy_data.common.cpath)
    regex4 := replace(regex3, "$(bundle-id)", "[a-z0-9]{64}")
    print("CopyFileRequest: regex4 =", regex4)

    regex.match(regex4, input.path)

    print("CopyFileRequest: true")
}

CreateSandboxRequest if {
    print("CreateSandboxRequest: input.guest_hook_path =", input.guest_hook_path)
    count(input.guest_hook_path) == 0

    print("CreateSandboxRequest: input.kernel_modules =", input.kernel_modules)
    count(input.kernel_modules) == 0

    i_pidns := input.sandbox_pidns
    print("CreateSandboxRequest: i_pidns =", i_pidns)
    i_pidns == false
    allow_sandbox_storages(input.storages)
}

allow_exec(p_container, i_process) if {
    print("allow_exec: start")

    p_oci = p_container.OCI
    p_s_name = p_oci.Annotations[S_NAME_KEY]
    s_namespace = get_state_val("namespace")
    allow_probe_process(p_oci.Process, i_process, p_s_name, s_namespace)

    print("allow_exec: true")
}

allow_interactive_exec(p_container, i_process) if {
    print("allow_interactive_exec: start")

    p_oci = p_container.OCI
    p_s_name = p_oci.Annotations[S_NAME_KEY]
    s_namespace = get_state_val("namespace")
    allow_interactive_process(p_oci.Process, i_process, p_s_name, s_namespace)

    print("allow_interactive_exec: true")
}

get_state_container(container_id):= p_container if {
    idx := get_state_val(container_id)
    p_container := policy_data.containers[idx]
}

ExecProcessRequest if {
    print("ExecProcessRequest 1: input =", input)
    allow_exec_process_input

    some p_command in policy_data.request_defaults.ExecProcessRequest.allowed_commands
    print("ExecProcessRequest 1: p_command =", p_command)
    p_command == input.process.Args

    p_container := get_state_container(input.container_id)
    allow_interactive_exec(p_container, input.process)

    print("ExecProcessRequest 1: true")
}
ExecProcessRequest if {
    print("ExecProcessRequest 2: input =", input)
    allow_exec_process_input

    p_container := get_state_container(input.container_id)

    some p_command in p_container.exec_commands
    print("ExecProcessRequest 2: p_command =", p_command)

    p_command == input.process.Args

    allow_exec(p_container, input.process)

    print("ExecProcessRequest 2: true")
}
ExecProcessRequest if {
    print("ExecProcessRequest 3: input =", input)
    allow_exec_process_input

    i_command = concat(" ", input.process.Args)
    print("ExecProcessRequest 3: i_command =", i_command)

    some p_regex in policy_data.request_defaults.ExecProcessRequest.regex
    print("ExecProcessRequest 3: p_regex =", p_regex)

    regex.match(p_regex, i_command)

    p_container := get_state_container(input.container_id)

    allow_interactive_exec(p_container, input.process)

    print("ExecProcessRequest 3: true")
}

allow_exec_process_input if {
    is_null(input.string_user)

    i_process := input.process
    count(i_process.SelinuxLabel) == 0
    count(i_process.ApparmorProfile) == 0

    print("allow_exec_process_input: true")
}

UpdateRoutesRequest if {
    print("UpdateRoutesRequest: input =", input)
    print("UpdateRoutesRequest: policy =", policy_data.request_defaults.UpdateRoutesRequest)

    i_routes := input.routes.Routes
    p_source_regex = policy_data.request_defaults.UpdateRoutesRequest.forbidden_source_regex
    p_names = policy_data.request_defaults.UpdateRoutesRequest.forbidden_device_names

    every i_route in i_routes {
        print("i_route.source =", i_route.source)
        every p_regex in p_source_regex {
            print("p_regex =", p_regex)
            not regex.match(p_regex, i_route.source)
        }

        print("i_route.device =", i_route.device)
        not i_route.device in p_names
    }

    print("UpdateRoutesRequest: true")
}

UpdateInterfaceRequest if {
    print("UpdateInterfaceRequest: input =", input)
    print("UpdateInterfaceRequest: policy =", policy_data.request_defaults.UpdateInterfaceRequest)

    i_interface := input.interface
    p_flags := policy_data.request_defaults.UpdateInterfaceRequest.allow_raw_flags

    # Typically, just IFF_NOARP is used.
    bits.and(i_interface.raw_flags, bits.negate(p_flags)) == 0

    p_names := policy_data.request_defaults.UpdateInterfaceRequest.forbidden_names

    not i_interface.name in p_names

    p_hwaddrs := policy_data.request_defaults.UpdateInterfaceRequest.forbidden_hw_addrs

    not i_interface.hwAddr in p_hwaddrs

    print("UpdateInterfaceRequest: true")
}

AddARPNeighborsRequest if {
    p_defaults := policy_data.request_defaults.AddARPNeighborsRequest
    print("AddARPNeighborsRequest: policy =", p_defaults)

    every i_neigh in input.neighbors.ARPNeighbors {
        print("AddARPNeighborsRequest: i_neigh =", i_neigh)

        not i_neigh.device in p_defaults.forbidden_device_names
        i_neigh.toIPAddress.mask == ""
        every p_cidr in p_defaults.forbidden_cidrs_regex {
            not regex.match(p_cidr, i_neigh.toIPAddress.address)
        }
        i_neigh.state == 128
        bits.or(i_neigh.flags, 136) == 136
    }

    print("AddARPNeighborsRequest: true")
}

CloseStdinRequest if {
    policy_data.request_defaults.CloseStdinRequest == true
}

ReadStreamRequest if {
    policy_data.request_defaults.ReadStreamRequest == true
}

UpdateEphemeralMountsRequest if {
    policy_data.request_defaults.UpdateEphemeralMountsRequest == true
}

WriteStreamRequest if {
    policy_data.request_defaults.WriteStreamRequest == true
}

RemoveContainerRequest:= {"ops": ops, "allowed": true} if {
    print("RemoveContainerRequest: input =", input)

    # Delete input.container_id from p_state
    ops_builder1 := []
    del_container := state_del_key(input.container_id)
    ops := concat_op_if_not_null(ops_builder1, del_container)

    print("RemoveContainerRequest: true")
}
