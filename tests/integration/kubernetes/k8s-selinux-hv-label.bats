#!/usr/bin/env bats

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

ensure_selinux() {
  # Install SELinux tools on Debian/Ubuntu-based systems
  sudo apt-get update || true
  sudo apt-get install -y selinux-utils policycoreutils selinux-basics selinux-policy-default || true

  state=$(getenforce)
  case "$state" in
    Enforcing|Permissive)
      ;;
    Disabled|disabled)
      skip "SELinux is disabled, skipping test"
      ;;
  esac

  # Flip 'enable_selinux = false' to 'true' (scoped broadly; assumes line exists)
  cfg="/etc/containerd/config.toml"
  sudo sed -i 's/^[[:space:]]*enable_selinux[[:space:]]*=[[:space:]]*false/  enable_selinux = true/' "$cfg" || true
  sudo systemctl restart containerd || true

}

# Extract SELinux fields from pod YAML. If all four fields exist, build
# user:role:type:level; otherwise fall back to just level (default s0).
get_expected_label_from_pod_yaml() {
  local yaml_file="$1"

  # Read raw values from YAML
  local raw_user raw_role raw_type raw_level
  raw_user=$(yq -r '.spec.securityContext.seLinuxOptions.user // ""' "${yaml_file}")
  raw_role=$(yq -r '.spec.securityContext.seLinuxOptions.role // ""' "${yaml_file}")
  raw_type=$(yq -r '.spec.securityContext.seLinuxOptions.type // ""' "${yaml_file}")
  raw_level=$(yq -r '.spec.securityContext.seLinuxOptions.level // ""' "${yaml_file}")

  # If all are empty, skip the test
  if [ -z "${raw_user}${raw_role}${raw_type}${raw_level}" ]; then
    skip "seLinuxOptions not set in YAML; skipping test"
  fi

  # Export variables used by the test
  sel_user="$raw_user"
  sel_role="$raw_role"
  sel_type="$raw_type"
  if [ -n "$raw_level" ]; then
    mcs_level="$raw_level"
  else
    mcs_level="s0"
  fi

  # Build expected label: only if YAML had all four fields
  if [ -n "$raw_user" ] && [ -n "$raw_role" ] && [ -n "$raw_type" ] && [ -n "$raw_level" ]; then
    expected_label="${sel_user}:${sel_role}:${sel_type}:${raw_level}"
  else
    expected_label="$mcs_level"
  fi
}

setup() {
  # Ensure host has SELinux available (Enforcing or Permissive); otherwise skip
  ensure_selinux

  get_pod_config_dir
  yaml_file="${pod_config_dir}/pod-selinux-hv.yaml"

  get_expected_label_from_pod_yaml "${yaml_file}"
}

@test "SELinux: hypervisor label matches YAML level" {
  pod_name="selinux-hv-test"

  # Create pod and wait for ready
  kubectl create -f "${yaml_file}"
  kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

  # Check host processes for hypervisor and optional virtiofsd and verify SELinux context
  lines=$(ps -eZ | grep -E "qemu|cloud-hypervisor|firecracker" || true)
  vfs_lines=$(ps -eZ | grep -E "virtiofsd" || true)

  [ -n "$lines" ]
  if [ -n "${expected_label}" ]; then
    echo "$lines" | grep -F "${expected_label}" >/dev/null
  else
    echo "$lines" | grep -F "$mcs_level" >/dev/null
  fi

  # virtiofsd is optional: if present, also verify; if absent, skip without failing
  if [ -n "$vfs_lines" ]; then
    if [ -n "${expected_label}" ]; then
      echo "$vfs_lines" | grep -F "${expected_label}" >/dev/null
    else
      echo "$vfs_lines" | grep -F "$mcs_level" >/dev/null
    fi
  fi
}

teardown() {
  [ -n "${pod_name:-}" ] || return 0
  kubectl describe "pod/$pod_name" || true
  kubectl delete pod "$pod_name" --ignore-not-found
}
