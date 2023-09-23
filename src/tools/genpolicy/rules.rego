package agent_policy

import future.keywords.in
import future.keywords.every

import input

# Requests that are always allowed.
default CreateSandboxRequest := true
default DestroySandboxRequest := true
default GetOOMEventRequest := true
default GuestDetailsRequest := true
default OnlineCPUMemRequest := true
default PullImageRequest := true
default RemoveContainerRequest := true
default RemoveStaleVirtiofsShareMountsRequest := true
default SignalProcessRequest := true
default StartContainerRequest := true
default StatsContainerRequest := true
default TtyWinResizeRequest := true
default UpdateEphemeralMountsRequest := true
default UpdateInterfaceRequest := true
default UpdateRoutesRequest := true
default WaitProcessRequest := true

# Configure the Agent to *allow any requests causing a policy failure*.
# This is an unsecure configuration but is useful for allowing unsecure
# pods to start, then connect to them and inspect OPA logs for the root
# cause of a failure.
# default AllowRequestsFailingPolicy := true

######################################################################
CreateContainerRequest {
    some policy_container in policy_data.containers

    policy_oci := policy_container.OCI
    policy_storages := policy_container.storages

    input_oci := input.OCI
    input_storages := input.storages

    print("==============================================")
    print("CreateContainerRequest: oci Version")
    policy_oci.Version     == input_oci.Version

    print("CreateContainerRequest: policy_oci.Root.Readonly")
    policy_oci.Root.Readonly  == input_oci.Root.Readonly

    print("CreateContainerRequest: allow annotations")
    allow_annotations(policy_oci, input_oci)

    print("CreateContainerRequest: allow_by_annotations")
    allow_by_annotations(policy_oci, input_oci, policy_storages, input_storages)

    print("CreateContainerRequest: allow_linux")
    allow_linux(policy_oci, input_oci)

    print("CreateContainerRequest: success")
}

######################################################################
# Reject unexpected annotations.
allow_annotations(policy_oci, input_oci) {
    not input_oci.Annotations
}
allow_annotations(policy_oci, input_oci) {
    input_keys := object.keys(input_oci.Annotations)

    every input_key in input_keys {
        print("allow_annotations: checking input key =", input_key)
        allow_annotation_key(input_key, policy_oci)
    }
}

allow_annotation_key(input_key, policy_oci) {
    startswith(input_key, "io.kubernetes.cri.")
}
allow_annotation_key(input_key, policy_oci) {
    some policy_key, _ in policy_oci.Annotations
    policy_key == input_key
}


######################################################################
# Get "io.kubernetes.cri.sandbox-name", and correlate its value with other
# annotations and process fields.

allow_by_annotations(policy_oci, input_oci, policy_storages, input_storages) {
    print("allow_by_annotations 1: no io.kubernetes.cri.sandbox-name in policy")
    not policy_oci.Annotations["io.kubernetes.cri.sandbox-name"]

    input_sandbox_name := input_oci.Annotations["io.kubernetes.cri.sandbox-name"]

    print("allow_by_annotations 1: allow_by_sandbox_name", input_sandbox_name)
    allow_by_sandbox_name(policy_oci, input_oci, policy_storages, input_storages, input_sandbox_name)

    print("allow_by_annotations 1: success")
}
allow_by_annotations(policy_oci, input_oci, policy_storages, input_storages) {
    print("allow_by_annotations 2: io.kubernetes.cri.sandbox-name")
    policy_sandbox_name := policy_oci.Annotations["io.kubernetes.cri.sandbox-name"]
    input_sandbox_name := input_oci.Annotations["io.kubernetes.cri.sandbox-name"]

    print("allow_by_annotations 2: input sandbox =", input_sandbox_name, "policy sandbox =", policy_sandbox_name)
    allow_sandbox_name(policy_sandbox_name, input_sandbox_name)

    print("allow_by_annotations 2: allow_by_sandbox_name", input_sandbox_name)
    allow_by_sandbox_name(policy_oci, input_oci, policy_storages, input_storages, input_sandbox_name)

    print("allow_by_annotations 2: success")
}

allow_by_sandbox_name(policy_oci, input_oci, policy_storages, input_storages, sandbox_name) {
    print("allow_by_sandbox_name: starting")

    policy_namespace := policy_oci.Annotations["io.kubernetes.cri.sandbox-namespace"]
    input_namespace := input_oci.Annotations["io.kubernetes.cri.sandbox-namespace"]
    print("allow_by_sandbox_name: policy_namespace =", policy_namespace, "input_namespace =", input_namespace)
    policy_namespace == input_namespace

    print("allow_by_sandbox_name: allow_by_container_types")
    allow_by_container_types(policy_oci, input_oci, sandbox_name, policy_namespace)

    print("allow_by_sandbox_name: allow_by_bundle_or_sandbox_id")
    allow_by_bundle_or_sandbox_id(policy_oci, input_oci, policy_storages, input_storages)

    print("allow_by_sandbox_name: allow_process")
    allow_process(policy_oci, input_oci, sandbox_name)

    print("allow_by_sandbox_name: success")
}

allow_sandbox_name(policy_sandbox_name, input_sandbox_name) {
    print("allow_sandbox_name 1: same name")
    policy_sandbox_name == input_sandbox_name
    print("allow_sandbox_name 1: success")
}
allow_sandbox_name(policy_sandbox_name, input_sandbox_name) {
    print("allow_sandbox_name 2: generated name")

    # TODO: should generated names be handled differently?
    contains(policy_sandbox_name, "$(generated-name)")

    print("allow_sandbox_name 2: success")
}
######################################################################
# - Check that the "io.kubernetes.cri.container-type" and
#   "io.katacontainers.pkg.oci.container_type" annotations
#   designate the expected type - either a "sandbox" or a
#   "container" type.
#
# - Then, validate other annotations based on the actual
#   "sandbox" or "container" value from the input container.

allow_by_container_types(policy_oci, input_oci, sandbox_name, sandbox_namespace) {
    print("allow_by_container_types: checking io.kubernetes.cri.container-type")
    
    policy_cri_type := policy_oci.Annotations["io.kubernetes.cri.container-type"]
    print("allow_by_container_types: policy type =", policy_cri_type)
    
    input_cri_type := input_oci.Annotations["io.kubernetes.cri.container-type"]
    print("allow_by_container_types: input type =", input_cri_type)
    
    policy_cri_type == input_cri_type

    print("allow_by_container_types: allow_by_container_type")
    allow_by_container_type(input_cri_type, policy_oci, input_oci, sandbox_name, sandbox_namespace)

    print("allow_by_container_types: success")
}

# Rules applicable to the "sandbox" container type
allow_by_container_type(input_cri_type, policy_oci, input_oci, sandbox_name, sandbox_namespace) {
    print("allow_by_container_type 1: input_cri_type =", input_cri_type)
    input_cri_type == "sandbox"

    input_kata_type := input_oci.Annotations["io.katacontainers.pkg.oci.container_type"]
    print("allow_by_container_type 1: input container type", input_kata_type)
    input_kata_type == "pod_sandbox"

    allow_sandbox_container_name(policy_oci, input_oci)
    allow_sandbox_net_namespace(policy_oci, input_oci)
    allow_sandbox_log_directory(policy_oci, input_oci, sandbox_name, sandbox_namespace)

    print("allow_by_container_type 1: success")
}

# Rules applicable to the "container" container type
allow_by_container_type(input_cri_type, policy_oci, input_oci, sandbox_name, sandbox_namespace) {
    print("allow_by_container_type 2: input_cri_type =", input_cri_type)
    input_cri_type == "container"

    input_kata_type := input_oci.Annotations["io.katacontainers.pkg.oci.container_type"]
    print("allow_by_container_type 2: input type", input_kata_type)
    input_kata_type == "pod_container"

    print("allow_by_container_type 2: allow_container_name")
    allow_container_name(policy_oci, input_oci)

    print("allow_by_container_type 2: allow_net_namespace")
    allow_net_namespace(policy_oci, input_oci)

    print("allow_by_container_type 2: allow_log_directory")
    allow_log_directory(policy_oci, input_oci)

    print("allow_by_container_type 2: success")
}

######################################################################
# "io.kubernetes.cri.container-name" annotation

allow_sandbox_container_name(policy_oci, input_oci) {
    print("allow_sandbox_container_name: container_annotation_missing")
    container_annotation_missing(policy_oci, input_oci, "io.kubernetes.cri.container-name")
    print("allow_sandbox_container_name: success")
}

allow_container_name(policy_oci, input_oci) {
    print("allow_container_name: allow_container_annotation")
    allow_container_annotation(policy_oci, input_oci, "io.kubernetes.cri.container-name")
    print("allow_container_name: success")
}

######################################################################
# Annotions required for "container" type, and not allowed for "sandbox" type.

container_annotation_missing(policy_oci, input_oci, annotation_key) {
    print("container_annotation_missing:", annotation_key)

    not policy_oci.Annotations[annotation_key]
    not input_oci.Annotations[annotation_key]

    print("container_annotation_missing: success")
}

allow_container_annotation(policy_oci, input_oci, annotation_key) {
    print("allow_container_annotation: annotation_key =", annotation_key)

    policy_value := policy_oci.Annotations[annotation_key]
    print("allow_container_annotation: policy_value =", policy_value)

    input_value := input_oci.Annotations[annotation_key]
    print("allow_container_annotation: input_value = ", input_value)

    policy_value == input_value
    print("allow_container_annotation: success")
}

######################################################################
# "nerdctl/network-namespace" annotation

allow_sandbox_net_namespace(policy_oci, input_oci) {
    print("allow_sandbox_net_namespace: start")

    policy_namespace := policy_oci.Annotations["nerdctl/network-namespace"]
    print("allow_sandbox_net_namespace: policy_namespace =", policy_namespace)

    input_namespace := input_oci.Annotations["nerdctl/network-namespace"]
    print("allow_sandbox_net_namespace: input_namespace =", input_namespace)

    regex.match(policy_namespace, input_namespace)
    print("allow_sandbox_net_namespace: success")
}

allow_net_namespace(policy_oci, input_oci) {
    print("allow_net_namespace: start")

    not policy_oci.Annotations["nerdctl/network-namespace"]
    not input_oci.Annotations["nerdctl/network-namespace"]

    print("allow_net_namespace: success")
}

######################################################################
# "io.kubernetes.cri.sandbox-log-directory" annotation

allow_sandbox_log_directory(policy_oci, input_oci, sandbox_name, sandbox_namespace) {
    print("allow_sandbox_log_directory: start")

    policy_log_directory := policy_oci.Annotations["io.kubernetes.cri.sandbox-log-directory"]
    directory_regex_tmp := replace(policy_log_directory, "$(sandbox-name)", sandbox_name)
    directory_regex := replace(directory_regex_tmp, "$(sandbox-namespace)", sandbox_namespace)
    print("allow_sandbox_log_directory: policy regex =", directory_regex)

    input_log_directory := input_oci.Annotations["io.kubernetes.cri.sandbox-log-directory"]
    print("allow_sandbox_log_directory: input =", input_log_directory)

    regex.match(directory_regex, input_log_directory)

    print("allow_sandbox_log_directory: success")
}

allow_log_directory(policy_oci, input_oci) {
    not policy_oci.Annotations["io.kubernetes.cri.sandbox-log-directory"]
    not input_oci.Annotations["io.kubernetes.cri.sandbox-log-directory"]
}

######################################################################
# Validate the linux fields from config.json.

allow_linux(policy_oci, input_oci) {
    print("allow_linux: policy namespaces =", policy_oci.Linux.Namespaces)
    print("allow_linux: input namespaces =", input_oci.Linux.Namespaces)
    policy_oci.Linux.Namespaces     == input_oci.Linux.Namespaces

    print("allow_linux: allow_masked_paths")
    allow_masked_paths(policy_oci, input_oci)

    print("allow_linux: allow_readonly_paths")
    allow_readonly_paths(policy_oci, input_oci)

    print("allow_linux: success")
}

######################################################################
allow_masked_paths(policy_oci, input_oci) {
    print("allow_masked_paths 1: policy maskedPaths =", policy_oci.Linux.MaskedPaths)
    print("allow_masked_paths 1: input maskedPaths =", input_oci.Linux.MaskedPaths)

    allow_masked_paths_array(policy_oci.Linux.MaskedPaths, input_oci.Linux.MaskedPaths)

    print("allow_masked_paths 1: success")
}
allow_masked_paths(policy_oci, input_oci) {
    print("allow_masked_paths 2: no maskedPaths")

    not policy_oci.Linux.MaskedPaths
    not input_oci.Linux.MaskedPaths

    print("allow_masked_paths 2: success")
}

# All the policy masked paths must be masked in the input data too.
# Input is allowed to have more masked paths than the policy.
allow_masked_paths_array(policy_array, input_array) {
    every policy_element in policy_array {
        allow_masked_path(policy_element, input_array)
    }
}

allow_masked_path(policy_element, input_array) {
    print("allow_masked_path: policy_element =", policy_element)

    some input_element in input_array
    policy_element == input_element

    print("allow_masked_path: success")
}

######################################################################
allow_readonly_paths(policy_oci, input_oci) {
    print("allow_readonly_paths 1: policy readonlyPaths =", policy_oci.Linux.ReadonlyPaths)
    print("allow_readonly_paths 1: input readonlyPaths =", input_oci.Linux.ReadonlyPaths)

    allow_readonly_paths_array(policy_oci.Linux.ReadonlyPaths, input_oci.Linux.ReadonlyPaths, input_oci.Linux.MaskedPaths)

    print("allow_readonly_paths 1: success")
}
allow_readonly_paths(policy_oci, input_oci) {
    print("allow_readonly_paths 2: no readonlyPaths")

    not policy_oci.Linux.ReadonlyPaths
    not input_oci.Linux.ReadonlyPaths

    print("allow_readonly_paths 2: success")
}

# All the policy readonly paths must be either:
# - Present in the input readonly paths, or
# - Present in the input masked paths.
# Input is allowed to have more readonly paths than the policy.
allow_readonly_paths_array(policy_array, input_array, masked_paths) {
    every policy_element in policy_array {
        allow_readonly_path(policy_element, input_array, masked_paths)
    }
}

allow_readonly_path(policy_element, input_array, masked_paths) {
    print("allow_readonly_path 1: policy_element =", policy_element)

    some input_element in input_array
    policy_element == input_element

    print("allow_readonly_path 1: success")
}
allow_readonly_path(policy_element, input_array, masked_paths) {
    print("allow_readonly_path 2: policy_element =", policy_element)

    some input_masked in masked_paths
    policy_element == input_masked

    print("allow_readonly_path 2: success")
}

######################################################################
# Get the input:
#
# - bundle_id from "io.katacontainers.pkg.oci.bundle_path"
# - sandbox_id from "io.kubernetes.cri.sandbox-id"
#
# and check their consistency with other rules.

allow_by_bundle_or_sandbox_id(policy_oci, input_oci, policy_storages, input_storages) {
    print("allow_by_bundle_or_sandbox_id: checking io.katacontainers.pkg.oci.bundle_path")
    bundle_path := input_oci.Annotations["io.katacontainers.pkg.oci.bundle_path"]
    bundle_id := replace(bundle_path, "/run/containerd/io.containerd.runtime.v2.task/k8s.io/", "")

    policy_sandbox_regex := policy_oci.Annotations["io.kubernetes.cri.sandbox-id"]
    sandbox_id := input_oci.Annotations["io.kubernetes.cri.sandbox-id"]

    print("allow_by_bundle_or_sandbox_id: regex.match sandbox_id =", sandbox_id, "regex =", policy_sandbox_regex)
    regex.match(policy_sandbox_regex, sandbox_id)

    print("allow_by_bundle_or_sandbox_id: allow_root_path")
    allow_root_path(policy_oci, input_oci, bundle_id)

    every input_mount in input.OCI.Mounts {
        print("allow_by_bundle_or_sandbox_id: allow_mount")
        allow_mount(policy_oci, input_mount, bundle_id, sandbox_id)
    }

    print("allow_by_bundle_or_sandbox_id: allow_storages")
    allow_storages(policy_storages, input_storages, bundle_id, sandbox_id)

    print("allow_by_bundle_or_sandbox_id: success")
}

######################################################################
# Validate the process fields from config.json.

allow_process(policy_oci, input_oci, sandbox_name) {
    policy_process := policy_oci.Process
    input_process := input_oci.Process

    print("allow_process: input terminal =", input_process.Terminal, "policy terminal =", policy_process.Terminal)
    policy_process.Terminal         == input_process.Terminal

    print("allow_process: input cwd =", input_process.Cwd, "policy cwd =", policy_process.Cwd)
    policy_process.Cwd              == input_process.Cwd

    print("allow_process: input capabilities =", input_process.Capabilities)
    print("allow_process: policy capabilities =", policy_process.Capabilities)
    policy_process.Capabilities     == input_process.Capabilities

    print("allow_process: input noNewPrivileges =", input_process.NoNewPrivileges, "policy noNewPrivileges =", policy_process.NoNewPrivileges)
    policy_process.NoNewPrivileges  == input_process.NoNewPrivileges

    print("allow_process: allow_user")
    allow_user(policy_process, input_process)

    print("allow_process: allow_args")
    allow_args(policy_process, input_process, sandbox_name)

    print("allow_process: allow_env")
    allow_env(policy_process, input_process, sandbox_name)

    print("allow_process: success")
}

######################################################################
# OCI process.User field

allow_user(policy_process, input_process) {
    policy_user := policy_process.User
    input_user := input_process.User

    # TODO: track down the reason for mcr.microsoft.com/oss/bitnami/redis:6.0.8 being
    #       executed with uid = 0 despite having "User": "1001" in its container image
    #       config.
    #print("allow_user: input uid =", input_user.UID, "policy uid =", policy_user.UID)
    #policy_user.UID                 == input_user.UID

    # TODO: track down the reason for registry.k8s.io/pause:3.9 being
    #       executed with gid = 0 despite having "65535:65535" in its container image
    #       config.
    #print("allow_user: input gid =", input_user.GID, "policy gid =", policy_user.GID)
    #policy_user.GID                 == input_user.GID

    # TODO: compare the additionalGids field too after computing its value
    # based on /etc/passwd and /etc/group from the container image.
}

######################################################################
# OCI process.Args field

allow_args(policy_process, input_process, sandbox_name) {
    print("allow_args 1: no args")

    not policy_process.Args
    not input_process.Args

    print("allow_args 1: success")
}
allow_args(policy_process, input_process, sandbox_name) {
    print("allow_args 2: policy args =", policy_process.Args)
    print("allow_args 2: input args =", input_process.Args)

    count(policy_process.Args) == count(input_process.Args)

    every i, input_arg in input_process.Args {
        allow_arg(i, input_arg, policy_process, sandbox_name)
    }

    print("allow_args 2: success")
}

allow_arg(i, input_arg, policy_process, sandbox_name) {
    print("allow_arg 1: i =", i, "input_arg =", input_arg, "policy_arg =", policy_process.Args[i])

    policy_arg := replace(policy_process.Args[i], "$$", "$")
    input_arg == policy_arg

    print("allow_arg 1: success")
}
allow_arg(i, input_arg, policy_process, sandbox_name) {
    print("allow_arg 2: i =", i, "input_arg =", input_arg, "policy_arg =", policy_process.Args[i])

    # TODO: can $(node-name) be handled better?
    contains(policy_process.Args[i], "$(node-name)")

    print("allow_arg 2: success")
}
allow_arg(i, input_arg, policy_process, sandbox_name) {
    print("allow_arg 3: i =", i, "input_arg =", input_arg, "policy_arg =", policy_process.Args[i])

    policy_arg := replace(policy_process.Args[i], "$$", "$")
    expanded_arg = replace(policy_arg, "$(sandbox-name)", sandbox_name)
    print("allow_arg 3: expanded policy_arg =", expanded_arg)
    expanded_arg == input_arg

    print("allow_arg 3: success")
}

######################################################################
# OCI process.Env field

allow_env(policy_process, input_process, sandbox_name) {
    print("allow_env: policy env =", policy_process.Env)

    every env_var in input_process.Env {
        print("allow_env => allow_env_var:", env_var)
        allow_env_var(policy_process, input_process, env_var, sandbox_name)
    }

    print("allow_env: success")
}

# Allow input env variables that match with request_defaults.
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var regex 1: some allow_env_regex match env_var")

    some policy_var_regex in policy_data.request_defaults.CreateContainerRequest.allow_env_regex
    regex.match(policy_var_regex, env_var)

    print("allow_env_var regex 1: success")
}

# Allow input env variables that are present in the policy data too.
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 1: some policy_env_var == env_var")

    some policy_env_var in policy_process.Env
    policy_env_var == env_var

    print("allow_env_var 1: success")
}

# Match input with one of the policy variables, after substituting $(sandbox-name).
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 2: replace $(sandbox-name)")

    some policy_env_var in policy_process.Env
    policy_var = replace(policy_env_var, "$(sandbox-name)", sandbox_name)

    print("allow_env_var 2: input =", env_var, "policy =", policy_var)
    policy_var == env_var

    print("allow_env_var 2: success")
}

# Allow service-related env variables:

# "KUBERNETES_PORT_443_TCP_PROTO=tcp"
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 3: KUBERNETES_PORT_443_TCP_PROTO=tcp")

    name_value := split(env_var, "=")
    count(name_value) == 2

    name_value[1] == "tcp"

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 5
    name_components[components_count - 1] == "PROTO"
    name_components[components_count - 2] == "TCP"
    name_components[components_count - 4] == "PORT"
    port := name_components[components_count - 3]
    is_port(port)

    print("allow_env_var 3: success")
}

# "KUBERNETES_PORT_443_TCP_PORT=443"
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 4: KUBERNETES_PORT_443_TCP_PORT=443")

    name_value := split(env_var, "=")
    count(name_value) == 2

    port = name_value[1]
    is_port(port)

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 5
    name_components[components_count - 1] == "PORT"
    name_components[components_count - 2] == "TCP"
    name_components[components_count - 3] == port
    name_components[components_count - 4] == "PORT"

    print("allow_env_var 4: success")
}

# "KUBERNETES_PORT_443_TCP_ADDR=10.0.0.1"
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 5: KUBERNETES_PORT_443_TCP_ADDR=10.0.0.1")

    name_value := split(env_var, "=")
    count(name_value) == 2

    is_ip(name_value[1])

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 5
    name_components[components_count - 1] == "ADDR"
    name_components[components_count - 2] == "TCP"
    name_components[components_count - 4] == "PORT"
    port := name_components[components_count - 3]
    is_port(port)

    print("allow_env_var 5: success")
}

# "KUBERNETES_SERVICE_HOST=10.0.0.1",
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 6: KUBERNETES_SERVICE_HOST=10.0.0.1")

    name_value := split(env_var, "=")
    count(name_value) == 2

    is_ip(name_value[1])

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 3
    name_components[components_count - 1] == "HOST"
    name_components[components_count - 2] == "SERVICE"

    print("allow_env_var 6: success")
}

# "KUBERNETES_SERVICE_PORT=443",
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 7: KUBERNETES_SERVICE_PORT=443")

    name_value := split(env_var, "=")
    count(name_value) == 2

    is_port(name_value[1])

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 3
    name_components[components_count - 1] == "PORT"
    name_components[components_count - 2] == "SERVICE"

    print("allow_env_var 7: success")
}

# "KUBERNETES_SERVICE_PORT_HTTPS=443",
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 8: KUBERNETES_SERVICE_PORT_HTTPS=443")

    name_value := split(env_var, "=")
    count(name_value) == 2

    is_port(name_value[1])

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 4
    name_components[components_count - 1] == "HTTPS"
    name_components[components_count - 2] == "PORT"
    name_components[components_count - 3] == "SERVICE"

    print("allow_env_var 8: success")
}

# "KUBERNETES_PORT=tcp://10.0.0.1:443",
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 9: KUBERNETES_PORT=tcp://10.0.0.1:443")

    name_value := split(env_var, "=")
    count(name_value) == 2

    is_tcp_uri(name_value[1])

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 2
    name_components[components_count - 1] == "PORT"

    print("allow_env_var 9: success")
}

# "KUBERNETES_PORT_443_TCP=tcp://10.0.0.1:443",
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 10: KUBERNETES_PORT_443_TCP=tcp://10.0.0.1:443")

    name_value := split(env_var, "=")
    count(name_value) == 2

    name_components = split(name_value[0], "_")
    components_count := count(name_components)
    components_count >= 4
    name_components[components_count - 1] == "TCP"
    name_components[components_count - 3] == "PORT"
    port := name_components[components_count - 2]
    is_port(port)

    is_tcp_uri(name_value[1])
    value_components = split(name_value[1], ":")
    count(value_components) == 3
    value_components[2] == port

    print("allow_env_var 10: success")
}

# Allow fieldRef "fieldPath: status.podIP" values.
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 11: fieldPath: status.podIP")

    name_value := split(env_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some policy_env_var in policy_process.Env
    allow_pod_ip_var(name_value[0], policy_env_var)

    print("allow_env_var 11: success")
}

# Allow common fieldRef variables.
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 12: fieldRef")

    name_value := split(env_var, "=")
    count(name_value) == 2

    some policy_env_var in policy_process.Env
    policy_name_value := split(policy_env_var, "=")
    count(policy_name_value) == 2

    policy_name_value[0] == name_value[0]

    # TODO: should these be handled in a different way?
    always_allowed := ["$(host-name)", "$(node-name)", "$(pod-uid)"]
    some allowed in always_allowed
    contains(policy_name_value[1], allowed)

    print("allow_env_var 12: success")
}

# Allow fieldRef "fieldPath: status.hostIP" values.
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 13: fieldPath: status.hostIP")

    name_value := split(env_var, "=")
    count(name_value) == 2
    is_ip(name_value[1])

    some policy_env_var in policy_process.Env
    allow_host_ip_var(name_value[0], policy_env_var)

    print("allow_env_var 13: success")
}

# Allow resourceFieldRef values (e.g., "limits.cpu").
allow_env_var(policy_process, input_process, env_var, sandbox_name) {
    print("allow_env_var 14: resourceFieldRef")

    name_value := split(env_var, "=")
    count(name_value) == 2

    some policy_env_var in policy_process.Env
    policy_name_value := split(policy_env_var, "=")
    count(policy_name_value) == 2

    policy_name_value[0] == name_value[0]

    # TODO: should these be handled in a different way?
    always_allowed = ["$(resource-field)", "$(todo-annotation)"]
    some allowed in always_allowed
    contains(policy_name_value[1], allowed)

    print("allow_env_var 14: success")
}


allow_pod_ip_var(var_name, policy_env_var) {
    print("allow_pod_ip_var: var_name =", var_name, "policy_env_var =", policy_env_var)

    policy_name_value := split(policy_env_var, "=")
    count(policy_name_value) == 2

    policy_name_value[0] == var_name
    policy_name_value[1] == "$(pod-ip)"

    print("allow_pod_ip_var: success")
}

allow_host_ip_var(var_name, policy_env_var) {
    print("allow_host_ip_var: var_name =", var_name, "policy_env_var =", policy_env_var)

    policy_name_value := split(policy_env_var, "=")
    count(policy_name_value) == 2

    policy_name_value[0] == var_name
    policy_name_value[1] == "$(host-ip)"

    print("allow_host_ip_var: success")
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

is_port(value) {
    number = to_number(value)
    number >= 1
    number <= 65635
}

# E.g., "tcp://10.0.0.1:443"
is_tcp_uri(value) {
    components = split(value, "//")
    count(components) == 2
    components[0] == "tcp:"

    ip_and_port = split(components[1], ":")
    count(ip_and_port) == 2
    is_ip(ip_and_port[0])
    is_port(ip_and_port[1])
}

######################################################################
# OCI root.Path

allow_root_path(policy_oci, input_oci, bundle_id) {
    policy_path := replace(policy_oci.Root.Path, "$(bundle-id)", bundle_id)
    policy_path == input_oci.Root.Path
}

######################################################################
# mounts

allow_mount(policy_oci, input_mount, bundle_id, sandbox_id) {
    print("allow_mount: input_mount.destination =", input_mount.destination)

    some policy_mount in policy_oci.Mounts
    policy_mount_allows(policy_mount, input_mount, bundle_id, sandbox_id)

    # TODO: are there any other required policy checks for mounts - e.g.,
    #       multiple mounts with same source or destination?
}

policy_mount_allows(policy_mount, input_mount, bundle_id, sandbox_id) {
    print("policy_mount_allows 1: policy_mount =", policy_mount)
    print("policy_mount_allows 1: input_mount =", input_mount)

    policy_mount == input_mount

    print("policy_mount_allows 1 success")
}
policy_mount_allows(policy_mount, input_mount, bundle_id, sandbox_id) {
    print("policy_mount_allows 2: input_mount.destination =", input_mount.destination, "policy_mount.destination =", policy_mount.destination)
    policy_mount.destination    == input_mount.destination

    print("policy_mount_allows 2: input type =", input_mount.type_, "policy type =", policy_mount.type_)
    policy_mount.type_           == input_mount.type_

    print("policy_mount_allows 2: input options =", input_mount.options)
    print("policy_mount_allows 2: policy options =", policy_mount.options)
    policy_mount.options        == input_mount.options

    print("policy_mount_allows 2: policy_mount_source_allows")
    policy_mount_source_allows(policy_mount, input_mount, bundle_id, sandbox_id)

    print("policy_mount_allows 2: success")
}

policy_mount_source_allows(policy_mount, input_mount, bundle_id, sandbox_id) {
    # E.g., "source": "^/run/kata-containers/shared/containers/$(bundle-id)-[a-z0-9]{16}-resolv.conf$",
    policy_source_regex := replace(policy_mount.source, "$(bundle-id)", bundle_id)
    print("policy_mount_source_allows 1: policy_source_regex =", policy_source_regex)

    print("policy_mount_source_allows 1: input_mount.source=", input_mount.source)
    regex.match(policy_source_regex, input_mount.source)

    print("policy_mount_source_allows 1: success")
}
policy_mount_source_allows(policy_mount, input_mount, bundle_id, sandbox_id) {
    # E.g., "source": "^/run/kata-containers/shared/containers/$(sandbox-id)/rootfs/local/data$",
    policy_source_regex := replace(policy_mount.source, "$(sandbox-id)", sandbox_id)

    print("policy_mount_source_allows 2: policy_source_regex =", policy_source_regex, "input_mount.source=", input_mount.source)
    regex.match(policy_source_regex, input_mount.source)

    print("policy_mount_source_allows 2: success")
}

######################################################################
# Storages

allow_storages(policy_storages, input_storages, bundle_id, sandbox_id) {
    policy_count := count(policy_storages)
    input_count := count(input_storages)
    print("allow_storages: policy_count =", policy_count, "input_count =", input_count)
    policy_count == input_count

    some i, input_storage in input_storages
    allow_input_storage(i, input_storage, policy_storages, policy_count, bundle_id, sandbox_id)

    print("allow_storages: success")
}

allow_input_storage(i, input_storage, policy_storages, count, bundle_id, sandbox_id) {
    print("allow_input_storage: i =", i, "input_storage =", input_storage)

    policy_storage := policy_storages[i]
    print("allow_input_storage: i =", i, "policy_storage =", policy_storage)

    storages_match(policy_storage, input_storage, bundle_id, sandbox_id)

    # Stop when reaching the last element of the storages array.
    i == count - 1
}

storages_match(policy_storage, input_storage, bundle_id, sandbox_id) {
    print("storages_match: policy driver =", policy_storage.driver, "input driver =", input_storage.driver)

    policy_storage.driver           == input_storage.driver
    policy_storage.driver_options   == input_storage.driver_options
    policy_storage.fs_group         == input_storage.fs_group

    allow_storage_options(policy_storage.options, input_storage.options, policy_storage.driver)
    allow_mount_point(policy_storage, input_storage, bundle_id, sandbox_id)

    # TODO: validate the source field too.

    print("storages_match: success")
}

allow_storage_options(policy_options, input_options, driver) {
    policy_options == input_options
    print("allow_storage_options 1: success")
}
allow_storage_options(policy_options, input_options, driver) {
    print("allow_storage_options 2: driver =", driver)
    driver == "overlayfs"
    count(policy_options) == 2

    policy_ids := split(policy_options[0], ":")
    print("allow_storage_options 2: policy_ids =", policy_ids)

    policy_hashes := split(policy_options[1], ":")
    print("allow_storage_options 2: policy_hashes =", policy_hashes)

    policy_count := count(policy_ids)
    print("allow_storage_options 2: policy_count =", policy_count)
    policy_count >= 1
    policy_count == count(policy_hashes)

    input_count := count(input_options)
    print("allow_storage_options 2: input_count =", input_count)
    input_count == policy_count + 3

    print("allow_storage_options 2: input_options[0] =", input_options[0])
    input_options[0] == "io.katacontainers.fs-opt.layer-src-prefix=/var/lib/containerd/io.containerd.snapshotter.v1.tardev/layers"

    print("allow_storage_options 2: input_options[input_count - 2] =", input_options[input_count - 2])
    input_options[input_count - 2] == "io.katacontainers.fs-opt.overlay-rw"

    lowerdir := concat("=", ["lowerdir", policy_options[0]])
    print("allow_storage_options 2: lowerdir =", lowerdir)

    input_options[input_count - 1] == lowerdir
    print("allow_storage_options 2: input_options[input_count - 1] =", input_options[input_count - 1])

    every i, policy_id in policy_ids {
        allow_overlay_layer(i, policy_id, policy_hashes[i], input_options[i + 1])
    }
}

allow_overlay_layer(i, policy_id, policy_hash, input_option) {
    print("allow_overlay_layer: i =", i, "policy_id =", policy_id, "policy_hash =", policy_hash)
    print("allow_overlay_layer: i =", i, "input_option =", input_option)

    startswith(input_option, "io.katacontainers.fs-opt.layer=")
    input_value := replace(input_option, "io.katacontainers.fs-opt.layer=", "")
    input_value_decoded := base64.decode(input_value)
    print("allow_overlay_layer: i =", i, "input_value_decoded =", input_value_decoded)

    policy_suffix := concat("=", ["tar,ro,io.katacontainers.fs-opt.block_device=file,io.katacontainers.fs-opt.is-layer,io.katacontainers.fs-opt.root-hash", policy_hash])
    policy_value := concat(",", [policy_id, policy_suffix])
    print("allow_overlay_layer: i =", i, "policy_value =", policy_value)

    policy_value == input_value_decoded

    print("allow_overlay_layer: i =", i, "success")
}

allow_mount_point(policy_storage, input_storage, bundle_id, sandbox_id) {
    print("allow_mount_point 1: fstype == tar")
    policy_storage.fstype == "tar"

    print("allow_mount_point 1: policy_storage.mount_point == input_storage.mount_point")
    policy_storage.mount_point == input_storage.mount_point

    print("allow_mount_point 1: success")
}
allow_mount_point(policy_storage, input_storage, bundle_id, sandbox_id) {
    print("allow_mount_point 2: fstype == fuse3.kata-overlay")
    policy_storage.fstype == "fuse3.kata-overlay"

    policy_mount_point := replace(policy_storage.mount_point, "$(bundle-id)", bundle_id)
    print("allow_mount_point 2: policy_mount_point =", policy_mount_point)

    policy_mount_point == input_storage.mount_point

    print("allow_mount_point 2: success")
}
allow_mount_point(policy_storage, input_storage, bundle_id, sandbox_id) {
    print("allow_mount_point 3: fstype == local")
    policy_storage.fstype == "local"

    mount_point_regex := replace(policy_storage.mount_point, "$(sandbox-id)", sandbox_id)
    print("allow_mount_point 3: mount_point_regex =", mount_point_regex)

    regex.match(mount_point_regex, input_storage.mount_point)

    print("allow_mount_point 3: success")
}
allow_mount_point(policy_storage, input_storage, bundle_id, sandbox_id) {
    print("allow_mount_point 4: fstype == bind")
    policy_storage.fstype == "bind"

    mount_point_regex := replace(policy_storage.mount_point, "$(bundle-id)", bundle_id)
    print("allow_mount_point 4: mount_point_regex =", mount_point_regex)

    regex.match(mount_point_regex, input_storage.mount_point)

    print("allow_mount_point 4: success")
}

######################################################################
CopyFileRequest {
    print("CopyFileRequest: input.path =", input.path)

    some policy_regex in policy_data.request_defaults.CopyFileRequest
    regex.match(policy_regex, input.path)

    print("CopyFileRequest: success")
}

ExecProcessRequest {
    print("ExecProcessRequest 1: input =", input)

    input_command = concat(" ", input.process.Args)

    some policy_command in policy_data.request_defaults.ExecProcessRequest
    policy_command == input_command

    print("ExecProcessRequest 1: success")
}
ExecProcessRequest {
    print("ExecProcessRequest 2: input =", input)

    # TODO: match input container ID with its corresponding container.exec_commands.
    input_command = concat(" ", input.process.Args)

    some container in policy_data.containers
    some policy_command in container.exec_commands
    print("ExecProcessRequest 2: policy_command =", policy_command)

    # TODO: should other input data fields be validated as well?
    policy_command == input_command

    print("ExecProcessRequest 2: success")
}

ReadStreamRequest {
    policy_data.request_defaults.ReadStreamRequest == true
}

WriteStreamRequest {
    policy_data.request_defaults.WriteStreamRequest == true
}
