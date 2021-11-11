# How to build, run and test Kata CCv0

## Introduction and Background

In order to try and make building (locally) and demoing the Kata Containers `CCv0` code base as simple as possible I've
shared a script [`ccv0.sh`](./ccv0.sh). This script was originally my attempt to automate the steps of the 
[Developer Guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md) so that I could do
different sections of them repeatedly and reliably as I was playing around with make changes to different parts of the 
Kata code base. I then tried to weave in some of the [`tests/.ci`](https://github.com/kata-containers/tests/tree/main/.ci) 
scripts in order to have less duplicated code and to make it support for platforms. Finally I extended it to include some
calls to start kata pods in Kubernetes and call
[`agent-ctl`](https://github.com/kata-containers/kata-containers/tree/main/tools/agent-ctl) and [`ctr shim`] to test the
shim and agent endpoint for pull image on guest for the CCv0 roadmap.

At the time of writing we only have some basic Kata agent and containerd shim support for image offload included into the
[`CCv0` branch](https://github.com/kata-containers/kata-containers/tree/CCv0), so the testing is limited to this, but as
more functionality is added I'm hoping that this script can grow and expand to handle it.

*Disclaimer: This script has mostly just been used and tested by me ([@stevenhorsman](https://github.com/stevenhorsman)),*
*so there might be issues with it. I'm happy to try and help solve these if possible, but this shouldn't be considered a*
*fully supported process by the Kata Containers community.*

## Basic demo How-to

In order to build, and demo the CCv0 functionality, these are the steps I take:
> **Note**: I've only tested this script running as root
- Provision a new VM
    - *I choose a Ubuntu 20.04 8GB VM for this as I had one available. There are some dependences on apt-get installed*
    *packages, so these will need re-working to be compatible with other platforms.*
- Copy the script over to your VM *(I put it in the home directory)* and ensure it has execute permission by running 
`chmod u+x ccv0.sh`
- Optionally set up some environment variables
    - By default the script checks out the `CCv0` branches of the `kata-containers/kata-containers` and 
      `kata-containers/tests` repositories, but it is designed to be used to test of personal forks and branches as well. 
      If you want to build and run these you can export the `katacontainers_repo`, `katacontainers_branch`, `tests_repo`
      and `tests_branch` variables e.g. `export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=stevenh/agent-pull-image-endpoint && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=stevenh/add-ccvo-changes-to-build`
      before running the script.
- Run the full build process with `. ~/ccv0.sh -d build_and_install_all`
    - *I run this script sourced just so that the required installed components are accessible on the `PATH` to the rest*
      *of the process without having to reload the session.*
    - The steps that `build_and_install_all` takes is:
        - Checkout the git repos for the `tests` and `kata-containers` repos as specified by the environment variables
        (default to `CCv0` branches if they are not supplied)
        - Use the `tests/.ci` scripts to install the build dependencies
        - Build and install the Kata runtime
        - Configure Kata to use containerd and for debug to be enabled (including enabling console access to the 
        kata-runtime, which should only be done in development)
        - Create, build and install a rootfs for the Kata hypervisor to use. For 'CCv0' this is currently based on Ubuntu
        20.04 and has extra packages like `skopeo` and `umoci` added.
        - Build the Kata guest kernel
        - Install QEMU
        - Set up `agent-ctl` testing by building the binary and configuring a bundle directory for it
        - Initialising Kubernetes to use the VM as a single node cluster
    - The first time this runs it may take a while, but subsequent runs will be quicker as more things are already
      installed and they can be further cut down by not running all the above steps 
      [see "Additional script usage" below](#additional-script-usage)
    - *Depending on how where your VMs are and how IPs are shared you might possibly get an error during "Store custom* 
      *stress image in registry" from docker matching `ERROR: toomanyrequests: Too Many Requests`. In order to get around*
      *this log into docker hub with `docker login` and re-run the step with `. ~/ccv0.sh -d init_kubernetes`.*
- Check that your Kubernetes cluster has been correctly set-up: 
```
$ kubectl get nodes
NAME                              STATUS   ROLES                  AGE     VERSION
stevenh-ccv0-demo1.fyre.ibm.com   Ready    control-plane,master   3m33s   v1.21.1
```
- Create a kata pod:
```
$ ~/ccv0.sh -d create_kata_pod
pod/nginx-kata created
NAME         READY   STATUS              RESTARTS   AGE
nginx-kata   0/1     ContainerCreating   0          5s
```
- Wait a few seconds for pod to start
```
$ kubectl get pods
NAME         READY   STATUS    RESTARTS   AGE
nginx-kata   1/1     Running   0          29s
```
- Create a new terminal to the VM and open shell into kata container and check the `/run/kata-containers` directory
doesn't have a bundle unpack for container id `0123456789`:
```
$ ~/ccv0.sh -d open_kata_shell
bash-5.1# ls -al /run/kata-containers/
total 0
drwxr-xr-x 6 root root 120 Sep  6 09:44 .
drwxr-xr-x 8 root root 180 Sep  6 09:44 ..
drwxr-xr-x 3 root root 100 Sep  6 09:44 970af18fcef7e6e6f89fe1c4e77c23d647e18fae93b66303217e5d15996282d9
drwxr-xr-x 3 root root 100 Sep  6 09:44 ad20b902eb7fdf7b33dd6ca47e6c7805e2dcfcd534530f68a1b9e4973572ce1a
drwxr-xr-x 3 root root  80 Sep  6 09:44 sandbox
drwxr-xr-x 3 root root  60 Sep  6 09:44 shared
```
- In another new terminal open the kata console log for streaming:
```
$ ~/ccv0.sh -d open_kata_console
```
- In the first terminal run the pull image on guest command against the Kata agent, via the shim (`containerd-shim-kata-v2`).
This can be achieved using the [containerd](https://github.com/containerd/containerd) CLI tool, `ctr`, which can be used to
interact with the shim directly. The command takes the form
`ctr --namespace k8s.io shim --id <sandbox-id> pull-image <image> <new-container-id>` and can been run directly, or through
the `ccv0.sh` script to automatically fill in the variables:
  - Optionally set up some environment variables to set the image and credentials used:
    - By default the shim pull image test in `ccv0.sh` will use the image `registry.fedoraproject.org/fedora:latest`
      which requires no authentication. If you want to use a different image, first set the `PULL_IMAGE` environment
      variable e.g. `export PULL_IMAGE="docker.io/library/busybox:latest"`. Currently the containerd shim pull image
      code doesn't support using a container registry that requires authentication, so if this is required, see the 
      below steps to run the pull image command against the agent directly.
  - Run the pull image agent endpoint with `~/ccv0.sh -d shim_pull_image`: 
    - *For reasons, we think are related to the disk space remaining when unpacking the image bundle, sometimes the*
      *[unpack fails the first time and the sandbox crashes](https://github.com/kata-containers/kata-containers/issues/2876),*
      *but seems to work the second time and the pod will restart automatically, so just re-open the shell and console and*
      *re-run the shim_pull_image.*
```bash
$ ~/ccv0.sh shim_pull_image
Issuing command 'ctr --namespace k8s.io shim --id 4cabb081a0d90e0d041e433eebf315983ef2a945f1b1759dea51ea41ceff3ac8 pull-image registry.fedoraproject.org/fedora:latest 0123456789'
```
  - Alternatively you can issue the command directly to the kata-agent pull image endpoint, which also supports
    credentials in order to pull from an authenticated registry:
    - Optionally set up some environment variables to set the image and credentials used:
        - Set the `PULL_IMAGE` environment variable e.g. `export PULL_IMAGE="docker.io/library/busybox:latest"`
          if a specific container image is required.
        - If the container registry for the image requires authentication then this can be set with an environment
          variable `SOURCE_CREDS`. For example to use `docker.io` as an authenticated user first run 
          `export SOURCE_CREDS="<dockerhub username>:<dockerhub api key>"`
            > **Note**: the credentials support on the agent request is a tactical solution for the short-term
              proof of concept to allow more images to be pulled and tested. Once we have support for getting
              keys into the kata guest using the attestation-agent and/or KBS I'd expect container registry
              credentials to be looked up using that mechanism.
    - Run the pull image agent endpoint with `~/ccv0.sh -d agent_pull_image`: 
        - *For reasons, we think are related to the disk space remaining when unpacking the image bundle,*
          *sometimes the* 
          *[unpack fails the first time and the sandbox crashes](https://github.com/kata-containers/kata-containers/issues/2876),*
          *but seems to work the second time and the pod will restart automatically, so just re-open the shell and console*
          *and re-run the agent_pull_image.*
```
$ ~/ccv0.sh -d agent_pull_image
    Finished release [optimized] target(s) in 0.21s
{"msg":"announce","level":"INFO","ts":"2021-09-15T08:40:14.189360410-07:00","subsystem":"rpc","name":"kata-agent-ctl","pid":"830920","version":"0.1.0","source":"kata-agent-ctl","config":"Config { server_address: \"vsock://1970354082:1024\", bundle_dir: \"/tmp/bundle\", timeout_nano: 0, interactive: false, ignore_errors: false }"}
{"msg":"client setup complete","level":"INFO","ts":"2021-09-15T08:40:14.193639057-07:00","pid":"830920","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0","server-address":"vsock://1970354082:1024"}
{"msg":"Run command PullImage (1 of 1)","level":"INFO","ts":"2021-09-15T08:40:14.196643765-07:00","pid":"830920","source":"kata-agent-ctl","subsystem":"rpc","name":"kata-agent-ctl","version":"0.1.0"}
{"msg":"response received","level":"INFO","ts":"2021-09-15T08:40:43.828200633-07:00","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0","pid":"830920","response":""}
{"msg":"Command PullImage (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-15T08:40:43.828261708-07:00","subsystem":"rpc","pid":"830920","source":"kata-agent-ctl","version":"0.1.0","name":"kata-agent-ctl"}
```
- In the kata shell terminal you can see the container bundle has been created:
```
$ ls -al /run/kata-containers/0123456789
total 1216
drwx------  3 root root     120 Sep 15 15:40 .
drwxr-xr-x  7 root root     140 Sep 15 15:40 ..
-rw-r--r--  1 root root    3088 Sep 15 15:40 config.json
dr-xr-xr-x 18 root root     440 Aug  9 05:48 rootfs
-rw-r--r--  1 root root 1235681 Sep 15 15:40 sha256_6db7cf62a51ac7d5b573f7a61a855093ff82d7c1caaf1413e7b4730a20a172d0.mtree
-rw-r--r--  1 root root     372 Sep 15 15:40 umoci.json
```
- The console shell shows what has happened:
```
Getting image source signatures
…
…
Writing manifest to image destination
Storing signatures
…
…
Writing manifest to image destination
Storing signatures
   • unpacking bundle ...
   • unpack rootfs: /run/kata-containers/0123456789/rootfs
   • unpack layer: sha256:ecfb9899f4ce3412a027b88f47dfea56664b5d4bc35eaa0f12c94c671f8ba503
   • ... done
   • computing filesystem manifest ...
   • ... done
   • unpacked image bundle: /run/kata-containers/0123456789
{"msg":"cid is \"0123456789\"","level":"INFO","ts":"2021-09-15T15:40:30.097333785+00:00","subsystem":"rpc","pid":"56","version":"0.1.0","name":"kata-agent","source":"agent"}
{"msg":"target_path_bundle is \"/run/kata-containers/0123456789\"","level":"INFO","ts":"2021-09-15T15:40:30.099306235+00:00","version":"0.1.0","source":"agent","subsystem":"rpc","pid":"56","name":"kata-agent"}
{"msg":"handling signal","level":"INFO","ts":"2021-09-15T15:40:43.786343725+00:00","source":"agent","pid":"56","version":"0.1.0","subsystem":"signals","name":"kata-agent","signal":"SIGCHLD"}
```
- After the image has been pulling you can create a container using the bundle that was created in the pod sandbox:
```
$ ~/ccv0.sh -d agent_create_container
    Finished release [optimized] target(s) in 0.25s
{"msg":"announce","level":"INFO","ts":"2021-09-15T08:41:48.099561118-07:00","version":"0.1.0","name":"kata-agent-ctl","subsystem":"rpc","source":"kata-agent-ctl","pid":"831696","config":"Config { server_address: \"vsock://1970354082:1024\", bundle_dir: \"/tmp/bundle\", timeout_nano: 0, interactive: false, ignore_errors: false }"}
{"msg":"client setup complete","level":"INFO","ts":"2021-09-15T08:41:48.105513768-07:00","version":"0.1.0","subsystem":"rpc","source":"kata-agent-ctl","pid":"831696","name":"kata-agent-ctl","server-address":"vsock://1970354082:1024"}
{"msg":"Run command CreateContainer (1 of 1)","level":"INFO","ts":"2021-09-15T08:41:48.105700254-07:00","subsystem":"rpc","pid":"831696","version":"0.1.0","name":"kata-agent-ctl","source":"kata-agent-ctl"}
{"msg":"response received","level":"INFO","ts":"2021-09-15T08:41:48.153446454-07:00","subsystem":"rpc","pid":"831696","name":"kata-agent-ctl","source":"kata-agent-ctl","version":"0.1.0","response":""}
{"msg":"Command CreateContainer (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-15T08:41:48.153715145-07:00","name":"kata-agent-ctl","source":"kata-agent-ctl","subsystem":"rpc","pid":"831696","version":"0.1.0"}
```
- In the kata shell terminal you can check that a new process has been created with a timestamp matching the create request:
```
$ ps -ef --sort=start_time | tail -5
101           89      64  0 15:38 ?        00:00:00 nginx: worker process
root          90      56  0 15:39 pts/0    00:00:00 [bash]
root         112      56  0 15:41 pts/1    00:00:00 /usr/bin/kata-agent init
root         115      90  0 15:42 pts/0    00:00:00 ps -ef --sort=start_time
root         116      90  0 15:42 pts/0    00:00:00 tail -5
```
- The console shell shows what has happened:
```
{"msg":"receive createcontainer, spec: Spec { version: \"1.0.2-dev\", process: Some(Process { terminal: true, console_size: None, user: User { uid: 0, gid: 0, additional_gids: [], username: \"\" }, args: [\"/bin/sh\"], env: [], cwd: \"/\", capabilities: Some(LinuxCapabilities { bounding: [], effective: [], inheritable: [], permitted: [], ambient: [] }), rlimits: [], no_new_privileges: true, apparmor_profile: \"\", oom_score_adj: Some(0), selinux_label: \"\" }), root: Some(Root { path: \"/tmp/bundle/rootfs\", readonly: true }), hostname: \"\", mounts: [], hooks: None, annotations: {}, linux: Some(Linux { uid_mappings: [], gid_mappings: [], sysctl: {}, resources: None, cgroups_path: \"\", namespaces: [], devices: [], seccomp: None, rootfs_propagation: \"\", masked_paths: [], readonly_paths: [], mount_label: \"\", intel_rdt: None }), solaris: None, windows: None, vm: None }","level":"INFO","ts":"2021-09-15T15:41:48.065347407+00:00","version":"0.1.0","source":"agent","pid":"56","subsystem":"rpc","name":"kata-agent"}
{"msg":"Does the bundle exist true","level":"INFO","ts":"2021-09-15T15:41:48.086566070+00:00","source":"agent","subsystem":"rpc","version":"0.1.0","name":"kata-agent","pid":"56"}
{"msg":"The config_path is \"/run/kata-containers/0123456789/config.json\"","level":"INFO","ts":"2021-09-15T15:41:48.090261259+00:00","version":"0.1.0","source":"agent","pid":"56","subsystem":"rpc","name":"kata-agent"}
{"msg":"None","level":"INFO","ts":"2021-09-15T15:41:48.090339688+00:00","source":"agent","subsystem":"rpc","pid":"56","version":"0.1.0","name":"kata-agent"}
{"msg":"new cgroup_manager Manager { paths: {}, mounts: {}, cpath: \"/0123456789\", cgroup: Cgroup { subsystems: [CpuSet(CpuSetController { base: \"/sys/fs/cgroup\", path: \"/sys/fs/cgroup/0123456789\", v2: true }), Cpu(CpuController { base: \"/sys/fs/cgroup\", path: \"/sys/fs/cgroup/0123456789\", v2: true }), BlkIo(BlkIoController { base: \"/sys/fs/cgroup\", path: \"/sys/fs/cgroup/0123456789\", v2: true }), Mem(MemController { base: \"/sys/fs/cgroup\", path: \"/sys/fs/cgroup/0123456789\", v2: true }), Pid(PidController { base: \"/sys/fs/cgroup\", path: \"/sys/fs/cgroup/0123456789\", v2: true })], hier: V2 { root: \"/sys/fs/cgroup\" }, path: \"0123456789\" } }","level":"INFO","ts":"2021-09-15T15:41:48.090560333+00:00","pid":"56","name":"kata-agent","subsystem":"rpc","version":"0.1.0","source":"agent"}
{"msg":"before create console socket!","level":"INFO","ts":"2021-09-15T15:41:48.092456050+00:00","subsystem":"process","version":"0.1.0","pid":"56","name":"kata-agent","source":"agent"}
{"msg":"enter container.start!","level":"INFO","ts":"2021-09-15T15:41:48.092678830+00:00","cid":"0123456789","module":"rustjail","name":"kata-agent","version":"0.1.0","source":"agent","pid":"56","subsystem":"container","eid":"0123456789"}
{"msg":"exec fifo opened!","level":"INFO","ts":"2021-09-15T15:41:48.092780015+00:00","pid":"56","module":"rustjail","subsystem":"container","version":"0.1.0","name":"kata-agent","eid":"0123456789","cid":"0123456789","source":"agent"}
{"msg":"Continuing execution in temporary process, new child has pid: Pid(112)","level":"INFO","ts":"2021-09-15T15:41:48.095759313+00:00","pid":"56","cid":"0123456789","name":"kata-agent","version":"0.1.0","module":"rustjail","eid":"0123456789","action":"child process log","source":"agent","subsystem":"container"}
{"msg":"child pid: 112","level":"INFO","ts":"2021-09-15T15:41:48.098663894+00:00","version":"0.1.0","cid":"0123456789","subsystem":"container","pid":"56","eid":"0123456789","module":"rustjail","name":"kata-agent","source":"agent"}
{"msg":"try to send spec from parent to child","level":"INFO","ts":"2021-09-15T15:41:48.098765550+00:00","name":"kata-agent","pid":"56","subsystem":"container","source":"agent","version":"0.1.0","action":"join-namespaces","cid":"0123456789","module":"rustjail","eid":"0123456789"}
{"msg":"wait child received oci spec","level":"INFO","ts":"2021-09-15T15:41:48.098869579+00:00","eid":"0123456789","version":"0.1.0","source":"agent","name":"kata-agent","cid":"0123456789","subsystem":"container","module":"rustjail","action":"join-namespaces","pid":"56"}
{"msg":"temporary parent process exit successfully","level":"INFO","ts":"2021-09-15T15:41:48.099052287+00:00","cid":"0123456789","source":"agent","subsystem":"container","module":"rustjail","pid":"56","version":"0.1.0","name":"kata-agent","action":"child process log","eid":"0123456789"}
{"msg":"handling signal","level":"INFO","ts":"2021-09-15T15:41:48.099408118+00:00","pid":"56","source":"agent","version":"0.1.0","name":"kata-agent","subsystem":"signals","signal":"SIGCHLD"}
{"msg":"wait_status","level":"INFO","ts":"2021-09-15T15:41:48.099492163+00:00","subsystem":"signals","name":"kata-agent","source":"agent","pid":"56","version":"0.1.0","wait_status result":"Exited(Pid(110), 0)"}
{"msg":"child process start run","level":"INFO","ts":"2021-09-15T15:41:48.102315100+00:00","eid":"0123456789","pid":"56","name":"kata-agent","action":"child process log","subsystem":"container","source":"agent","module":"rustjail","cid":"0123456789","version":"0.1.0"}
{"msg":"notify parent to send oci process","level":"INFO","ts":"2021-09-15T15:41:48.102754152+00:00","source":"agent","version":"0.1.0","action":"child process log","module":"rustjail","cid":"0123456789","pid":"56","name":"kata-agent","eid":"0123456789","subsystem":"container"}
{"msg":"send oci process from parent to child","level":"INFO","ts":"2021-09-15T15:41:48.105592707+00:00","name":"kata-agent","source":"agent","cid":"0123456789","module":"rustjail","pid":"56","action":"join-namespaces","version":"0.1.0","eid":"0123456789","subsystem":"container"}
{"msg":"wait child received oci process","level":"INFO","ts":"2021-09-15T15:41:48.105709729+00:00","eid":"0123456789","source":"agent","name":"kata-agent","pid":"56","module":"rustjail","action":"join-namespaces","cid":"0123456789","subsystem":"container","version":"0.1.0"}
{"msg":"notify parent to send cgroup manager","level":"INFO","ts":"2021-09-15T15:41:48.105866282+00:00","cid":"0123456789","module":"rustjail","action":"child process log","eid":"0123456789","source":"agent","subsystem":"container","name":"kata-agent","version":"0.1.0","pid":"56"}
{"msg":"wait child setup user namespace","level":"INFO","ts":"2021-09-15T15:41:48.106040278+00:00","action":"join-namespaces","cid":"0123456789","source":"agent","subsystem":"container","module":"rustjail","pid":"56","version":"0.1.0","eid":"0123456789","name":"kata-agent"}
{"msg":"write oom score 0","level":"INFO","ts":"2021-09-15T15:41:48.106577659+00:00","pid":"56","version":"0.1.0","name":"kata-agent","cid":"0123456789","eid":"0123456789","action":"child process log","subsystem":"container","source":"agent","module":"rustjail"}
{"msg":"notify parent unshare user ns completed","level":"INFO","ts":"2021-09-15T15:41:48.106773826+00:00","version":"0.1.0","pid":"56","eid":"0123456789","module":"rustjail","source":"agent","action":"child process log","subsystem":"container","name":"kata-agent","cid":"0123456789"}
{"msg":"apply cgroups!","level":"INFO","ts":"2021-09-15T15:41:48.107248976+00:00","version":"0.1.0","source":"agent","eid":"0123456789","cid":"0123456789","action":"join-namespaces","name":"kata-agent","subsystem":"container","module":"rustjail","pid":"56"}
{"msg":"cgroup manager set resources for container. Resources input LinuxResources { devices: [LinuxDeviceCgroup { allow: false, type: \"b\", major: Some(259), minor: Some(1), access: \"rw\" }], memory: None, cpu: None, pids: None, block_io: None, hugepage_limits: [], network: None, rdma: {} }","level":"INFO","ts":"2021-09-15T15:41:48.107358603+00:00","source":"agent","version":"0.1.0","subsystem":"cgroups","name":"kata-agent","pid":"56"}
{"msg":"cgroup manager set devices","level":"INFO","ts":"2021-09-15T15:41:48.107545565+00:00","name":"kata-agent","subsystem":"cgroups","pid":"56","version":"0.1.0","source":"agent"}
{"msg":"resources after processed Resources { memory: MemoryResources { kernel_memory_limit: None, memory_hard_limit: None, memory_soft_limit: None, kernel_tcp_memory_limit: None, memory_swap_limit: None, swappiness: None, attrs: {} }, pid: PidResources { maximum_number_of_processes: None }, cpu: CpuResources { cpus: None, mems: None, shares: None, quota: None, period: None, realtime_runtime: None, realtime_period: None, attrs: {} }, devices: DeviceResources { devices: [DeviceResource { allow: false, devtype: Block, major: 259, minor: 1, access: [Read, Write] }, DeviceResource { allow: true, devtype: Char, major: 1, minor: 3, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 1, minor: 5, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 1, minor: 7, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 5, minor: 0, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 1, minor: 9, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 1, minor: 8, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: -1, minor: -1, access: [MkNod] }, DeviceResource { allow: true, devtype: Block, major: -1, minor: -1, access: [MkNod] }, DeviceResource { allow: true, devtype: Char, major: 5, minor: 1, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 136, minor: -1, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 5, minor: 2, access: [Read, Write, MkNod] }, DeviceResource { allow: true, devtype: Char, major: 10, minor: 200, access: [Read, Write, MkNod] }] }, network: NetworkResources { class_id: None, priorities: [] }, hugepages: HugePageResources { limits: [] }, blkio: BlkIoResources { weight: None, leaf_weight: None, weight_device: [], throttle_read_bps_device: [], throttle_read_iops_device: [], throttle_write_bps_device: [], throttle_write_iops_device: [] } }","level":"INFO","ts":"2021-09-15T15:41:48.107708982+00:00","version":"0.1.0","subsystem":"cgroups","name":"kata-agent","pid":"56","source":"agent"}
{"msg":"notify child to continue","level":"INFO","ts":"2021-09-15T15:41:48.108346455+00:00","action":"join-namespaces","cid":"0123456789","version":"0.1.0","subsystem":"container","eid":"0123456789","name":"kata-agent","module":"rustjail","source":"agent","pid":"56"}
{"msg":"notify child parent ready to run prestart hook!","level":"INFO","ts":"2021-09-15T15:41:48.108819006+00:00","source":"agent","pid":"56","version":"0.1.0","eid":"0123456789","module":"rustjail","action":"join-namespaces","name":"kata-agent","subsystem":"container","cid":"0123456789"}
{"msg":"get ready to run prestart hook!","level":"INFO","ts":"2021-09-15T15:41:48.108935516+00:00","subsystem":"container","eid":"0123456789","cid":"0123456789","action":"join-namespaces","module":"rustjail","version":"0.1.0","pid":"56","name":"kata-agent","source":"agent"}
{"msg":"notify child run prestart hook completed!","level":"INFO","ts":"2021-09-15T15:41:48.109044163+00:00","eid":"0123456789","source":"agent","pid":"56","name":"kata-agent","action":"join-namespaces","cid":"0123456789","module":"rustjail","version":"0.1.0","subsystem":"container"}
{"msg":"notify child parent ready to run poststart hook!","level":"INFO","ts":"2021-09-15T15:41:48.109152261+00:00","eid":"0123456789","action":"join-namespaces","subsystem":"container","name":"kata-agent","version":"0.1.0","pid":"56","cid":"0123456789","source":"agent","module":"rustjail"}
{"msg":"get ready to run poststart hook!","level":"INFO","ts":"2021-09-15T15:41:48.109260409+00:00","cid":"0123456789","eid":"0123456789","subsystem":"container","name":"kata-agent","action":"join-namespaces","pid":"56","source":"agent","module":"rustjail","version":"0.1.0"}
{"msg":"wait for child process ready to run exec","level":"INFO","ts":"2021-09-15T15:41:48.109368+00:00","version":"0.1.0","name":"kata-agent","cid":"0123456789","subsystem":"container","action":"join-namespaces","source":"agent","eid":"0123456789","module":"rustjail","pid":"56"}
{"msg":"entered namespaces!","level":"INFO","ts":"2021-09-15T15:41:48.109476291+00:00","version":"0.1.0","eid":"0123456789","module":"rustjail","name":"kata-agent","source":"agent","cid":"0123456789","subsystem":"container","pid":"56"}
{"msg":"updating namespaces","level":"INFO","ts":"2021-09-15T15:41:48.109569518+00:00","name":"kata-agent","pid":"56","subsystem":"container","module":"rustjail","source":"agent","version":"0.1.0","cid":"0123456789"}
{"msg":"wait on child log handler","level":"INFO","ts":"2021-09-15T15:41:48.109778744+00:00","name":"kata-agent","pid":"56","cid":"0123456789","version":"0.1.0","subsystem":"container","eid":"0123456789","module":"rustjail","source":"agent"}
{"msg":"wait parent to setup user id mapping","level":"INFO","ts":"2021-09-15T15:41:48.110246955+00:00","eid":"0123456789","source":"agent","version":"0.1.0","pid":"56","action":"child process log","cid":"0123456789","subsystem":"container","module":"rustjail","name":"kata-agent"}
{"msg":"setup rootfs /run/kata-containers/0123456789/rootfs","level":"INFO","ts":"2021-09-15T15:41:48.110375822+00:00","cid":"0123456789","source":"agent","name":"kata-agent","eid":"0123456789","version":"0.1.0","action":"child process log","subsystem":"container","module":"rustjail","pid":"56"}
{"msg":"process command: [\"/bin/sh\"]","level":"INFO","ts":"2021-09-15T15:41:48.110472642+00:00","subsystem":"container","eid":"0123456789","cid":"0123456789","action":"child process log","name":"kata-agent","pid":"56","source":"agent","version":"0.1.0","module":"rustjail"}
{"msg":"ready to run exec","level":"INFO","ts":"2021-09-15T15:41:48.110565466+00:00","eid":"0123456789","pid":"56","version":"0.1.0","subsystem":"container","cid":"0123456789","name":"kata-agent","source":"agent","module":"rustjail","action":"child process log"}
{"msg":"read child process log end","level":"INFO","ts":"2021-09-15T15:41:48.110795068+00:00","source":"agent","pid":"56","module":"rustjail","eid":"0123456789","action":"child process log","cid":"0123456789","subsystem":"container","name":"kata-agent","version":"0.1.0"}
{"msg":"create process completed","level":"INFO","ts":"2021-09-15T15:41:48.111369509+00:00","name":"kata-agent","subsystem":"container","eid":"0123456789","module":"rustjail","source":"agent","pid":"56","cid":"0123456789","version":"0.1.0"}
{"msg":"created container!","level":"INFO","ts":"2021-09-15T15:41:48.111684371+00:00","version":"0.1.0","source":"agent","pid":"56","subsystem":"rpc","name":"kata-agent"}
```
- Once complete you can clean up the kata pod by running:
```
$ ~/ccv0.sh -d delete_kata_pod
pod "nginx-kata" deleted
```

## Verifying signed images

> **Note**: the current proof of concept signature validation code involves hard-coding to protect a specific container 
repository and is only a temporary to demonstrate the function. After the attestation agent is able to pass through
trusted information and the [image management crate](https://github.com/confidential-containers/image-rs) is
implemented and integrated this code will be replaced.

For the proof of concept the ability to verify images is limited to a pre-created selection of test images in our test
repository [`quay.io/kata-containers/confidential-containers`](https://quay.io/repository/kata-containers/confidential-containers?tab=tags).
For pulling images not in this test repository (called an *unprotected* registry below), we can not currently get the GPG keys, or signatures used for signed images, so for compatibility we fall back to the behaviour of not enforcing signatures.


In our test repository there are three tagged images:

| Test Image | Base Image used | Signature status | GPG key status |
| --- | --- | --- | --- |
| `quay.io/kata-containers/confidential-containers:signed` | `busybox:1.33.1` | [signature](./../../tools/osbuilder/rootfs-builder/signed-container-artifacts/signatures.tar) embedded in kata rootfs |  [public key](./../../tools/osbuilder/rootfs-builder/signed-container-artifacts/public.gpg) embedded in kata rootfs |
| `quay.io/kata-containers/confidential-containers:unsigned` | `busybox:1.33.1` | not signed | not signed |
| `quay.io/kata-containers/confidential-containers:other_signed` | `nginx:1.21.3` | [signature](./../../tools/osbuilder/rootfs-builder/signed-container-artifacts/signatures.tar) embedded in kata rootfs | GPG key not kept |

Using a standard unsigned `busybox` image that can be pulled from `docker.io` we can test a few scenarios.

From this temporary proof of concept, along with the public GPG key and signature files, a container policy file is
created in the rootfs which specifies that any container image from `quay.io/kata-containers`
must be signed with the embedded GPG key. In order to enable this a new agent configuration parameter called
`policy_path` must been provided to the agent which specifies the location of the policy file to use inside the image. The `ccv0.sh`
script sets this up automatically by appending `agent.container_policy_file=/etc/containers/quay_verification/quay_policy.json`
to the `kernel_params` entry in `/etc/kata-containers/configuration.toml`.

With this policy parameter set a few tests of image verification can be done to test different scenarios
> **Note**: at the time of writing the `ctr shim` command has a [bug](https://github.com/kata-containers/kata-containers/issues/3020), so I'm using the agent commands directly through `agent-ctl` to drive the tests
- To test the fallback behaviour works using an unsigned image on an *unprotected* registry we can pull the `busybox`
image by running:
  ```bash
  export CONTAINER_ID="unprotected-unsigned"
  export PULL_IMAGE="docker.io/library/busybox:latest"
  ~/ccv0.sh -d agent_pull_image
  ```
  - This finishes with a return `Ok()` and after creating a shell into the Kata sandbox we can see that the container
  image was successfully unpacked to the correct place:
  ```
  # ls /run/kata-containers/unprotected-unsigned/
  config.json
  rootfs
  sha256_824b88c5c38e2b931cfd471061a576a2ac1c165ef7adeae6662687031b9f9e07.mtree
  umoci.json
  ```
- To test that an unsigned image from our *protected* test container registry is rejected we can run:
  ```bash
  export CONTAINER_ID="protected-unsigned"
  export PULL_IMAGE="quay.io/kata-containers/confidential-containers:unsigned"
  ~/ccv0.sh -d agent_pull_image
  ```
  - This results in an `ERROR: API failed` message from `agent_ctl` and the Kata sandbox console log shows the correct
  cause that the signature we has was not valid for the unsigned image:
  ```text
  FATA[0001] Source image rejected: Signature for identity quay.io/kata-containers/confidential-containers:signed is not accepted
  ```
- To test that the signed image our *protected* test container registry is accepted we can run:
  ```bash
  export CONTAINER_ID="protected-signed"
  export PULL_IMAGE="quay.io/kata-containers/confidential-containers:signed"
  ~/ccv0.sh -d agent_pull_image
  ```
  - This finishes with a return `Ok()` and again, creating a shell into the Kata sandbox we can see that the container
  image was successfully unpacked to the correct place:
  ```
  # ls /run/kata-containers/protected-signed/
  config.json
  rootfs
  sha256_ebf391d3f0ba36d4b64999ebbeadc878d229faec8839254a1c2264cf47735841.mtree
  umoci.json
  ```
- Finally to check the image with a valid signature, but invalid GPG key (the real trusted piece of information we really
want to protect with the attestation agent in future) fails we can run: 
  ```bash
  export CONTAINER_ID="protected-wrong-key"
  export PULL_IMAGE="quay.io/kata-containers/confidential-containers:other_signed"
  ~/ccv0.sh -d agent_pull_image
  ```
  - Again this results in an `ERROR: API failed` message from `agent_ctl` and the Kata sandbox console log shows a
  slightly different error:
  ```text
  FATA[0001] Source image rejected: Invalid GPG signature...
  ```
## Additional script usage

As well as being able to use the script as above to build all of `kata-containers` from scratch it can be used to just
re-build bits of it by running the script with different parameters. For example after the first build you will often
not need to re-install the dependencies, QEMU or the Guest kernel, but just test code changes made to the runtime and
agent. This can be done by running `. ~/ccv0.sh -d rebuild_and_install_kata`. (*Note this does a hard checkout*
*from git, so if your changes are only made locally it is better to do the individual steps e.g.* 
`. ~/ccv0.sh -d build_kata_runtime && . ~/ccv0.sh -d build_and_add_agent_to_rootfs && . ~/ccv0.sh -d build_and_install_rootfs`).
There are commands for a lot of steps in building, setting up and testing and the full list can be seen by running
`~/ccv0.sh help`:
```
$ ~/ccv0.sh help
Overview:
    Build and test kata containers from source
    Optionally set kata-containers and tests repo and branch as exported variables before running
    e.g. export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=kata-ci-from-fork && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=kata-ci-from-fork && . ~/ccv0.sh -d build_and_install_all
Usage:
    ccv0.sh [options] <command>
Commands:
- help:                         Display this help
- all:                          Build and install everything, test kata with containerd and capture the logs
- build_and_install_all:        Build and install everything
- initialise:                   Install dependencies and check out kata-containers source
- rebuild_and_install_kata:     Rebuild the kata runtime and agent and build and install the image
- build_kata_runtime:           Build and install the kata runtime
- configure:                    Configure Kata to use rootfs and enable debug
- create_rootfs:                Create a local rootfs
- build_and_add_agent_to_rootfs:Builds the kata-agent and adds it to the rootfs
- build_and_install_rootfs:     Builds and installs the rootfs image
- install_guest_kernel:         Setup, build and install the guest kernel
- build_qemu:                   Checkout, patch, build and install QEMU
- init_kubernetes:              initialise a Kubernetes cluster on this system
- create_kata_pod:              Create a kata runtime nginx pod in Kubernetes
- delete_kata_pod:              Delete a kata runtime nginx pod in Kubernetes
- open_kata_console:            Stream the kata runtime's console
- open_kata_shell:              Open a shell into the kata runtime
- agent_pull_image:             Run PullImage command against the agent with agent-ctl
- agent_create_container:       Run CreateContainer command against the agent with agent-ctl
- agent_list_commands:          List agent commands on agent-ctl
- test:                         Test using kata with containerd
- test_capture_logs:            Test using kata with containerd and capture the logs in the user's home directory

Options:
    -d: Enable debug
    -h: Display this help
```
