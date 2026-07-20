module github.com/kata-containers/kata-containers/tools/osbuilder/rootfs-builder/nvidia/kata-nvidia-cdi-list

go 1.25.12

require (
	github.com/NVIDIA/go-nvml v0.13.0-1
	// Must match semver in versions.yaml externals.nvidia.ctk.version (e.g. 1.18.1-1 -> v1.18.1). Checked in nvidia_rootfs.sh before go build.
	github.com/NVIDIA/nvidia-container-toolkit v1.18.1
)

require (
	github.com/NVIDIA/go-nvlib v0.8.1 // indirect
	github.com/fsnotify/fsnotify v1.7.0 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/kr/text v0.2.0 // indirect
	github.com/moby/sys/capability v0.4.0 // indirect
	github.com/opencontainers/runtime-spec v1.3.0 // indirect
	github.com/opencontainers/runtime-tools v0.9.1-0.20251114084447-edf4cb3d2116 // indirect
	github.com/sirupsen/logrus v1.9.3 // indirect
	golang.org/x/mod v0.29.0 // indirect
	golang.org/x/sys v0.44.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
	sigs.k8s.io/yaml v1.4.0 // indirect
	tags.cncf.io/container-device-interface v1.0.2-0.20251114135136-1b24d969689f // indirect
	tags.cncf.io/container-device-interface/specs-go v1.0.0 // indirect
)
