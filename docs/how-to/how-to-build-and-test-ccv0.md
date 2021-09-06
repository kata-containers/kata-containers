# How to build, run and test Kata CCv0

## Introduction and Background

In order to try and make building (locally) and demoing the Kata Containers `CCv0` code base as simple as possible I've shared a script [`ccv0.sh`](./ccv0.sh). This script was originally my attempt to automate the steps of the [Developer Guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md) so that I could do different sections of them repeatedly and reliably as I was playing around with make changes to different parts of the Kata code base. I then tried to weave in some of the [`tests/.ci`](https://github.com/kata-containers/tests/tree/main/.ci) scripts in order to have less duplicated code and to make it support for platforms. Finally I extended it to include some calls to start kata pods in Kubernetes and call [`agent-ctl`](https://github.com/kata-containers/kata-containers/tree/main/tools/agent-ctl) to test the agent endpoint for pull image on guest for the CCv0 roadmap.

At the time of writing we only have some basic Kata agent pull image support for CCv0 included into the [`CCv0` branch](https://github.com/kata-containers/kata-containers/tree/CCv0), so the testing is limited to this, but as more functionality is added I'm hoping that this script can grow and expand to handle it.

*Disclaimer: This script has mostly just been used and tested by me ([@stevenhorsman](https://github.com/stevenhorsman)), so there might be issues with it. I'm happy to try and help solve these if possible, but this shouldn't be considered a fully supported process by the Kata Containers community.*

## Basic demo How-to

In order to build, and demo the CCv0 functionality, these are the steps I take:
- Provision a new VM
    - *I choose a Ubuntu 20.04 8GB VM for this as I had one available. I think that the only part of the script that is OS dependent is the install of `git`, `socat` and `qemu-utils`(optional) using apt-get to bootstrap the rest of the installs. In order to run this on any platform just use your package manager to install these before running the `ccv0.sh` script and comment out the apt-get line with `sudo sed -i -e 's/\(sudo apt-get update .*\)$/# \1/g' ccv0.sh`*.
- Copy the script over to your VM *(I put it in the home directory)* and ensure it has execute permission by running `chmod u+x ccv0.sh`
- Optionally set up some environment variables
    - By default the script checks out the `CCv0` branches of the `kata-containers/kata-containers` and `kata-containers/tests` repositories, but it is designed to be used to test of personal forks and branches as well. If you want to build and run these you can export the `katacontainers_repo`, `katacontainers_branch`, `tests_repo` and `tests_branch` variables e.g. `export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=stevenh/agent-pull-image-endpoint && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=stevenh/add-ccvo-changes-to-build` before running the script.
- Run the full build process with `. ~/ccv0.sh -d build_and_install_all`
    - *I run this script sourced just so that the required installed components are accessible on the `PATH` to the rest of the process without having to reload the session.*
    - The steps that `build_and_install_all` takes is:
        - Checkout the git repos for the `tests` and `kata-containers` repos as specified by the environment variables (default to `CCv0` branches if they are not supplied)
        - Use the `tests/.ci` scripts to install the build dependencies
        - Build and install the Kata runtime
        - Configure Kata to use containerd and for debug to be enabled (including enabling console access to the kata-runtime, which should only be done in development)
        - Create, build and install a rootfs for the Kata hypervisor to use. For 'CCv0' this is currently based on Fedora 34 and has extra packages like `skopeo` and `umoci` added.
        - Build the Kata guest kernel
        - Install QEMU
        - Set up `agent-ctl` testing by building the binary and configuring a bundle directory for it
        - Initialising Kubernetes to use the VM as a single node cluster
    - The first time this runs it may take a while, but subsequent runs will be quicker as more things are already installed and they can be further cut down by not running all the above steps [see "Additional script usage" below](#additional-script-usage)
    - *Depending on how where your VMs are and how IPs are shared you might possibly get an error during "Store custom stress image in registry" from docker matching `ERROR: toomanyrequests: Too Many Requests`. In order to get around this log into docker hub with `sudo docker login` and re-run the step with `. ~/ccv0.sh -d init_kubernetes`.*
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
- Create a new terminal to the VM and open shell into kata container and check the `/run/kata-containers` directory doesn't have a bundle unpack for container id `0123456789`:
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
- In the first console list run the pull image agent endpoint using `~/ccv0.sh -d agent_pull_image`: 
    - *For unknown reasons sometimes the unpack fails the first time and the sandbox crashes, but seems to work the second time and the pod will restart automatically, so just re-open the shell and console and re-run the agent_pull_image.*
```
$ ~/ccv0.sh -d agent_pull_image
    Finished release [optimized] target(s) in 0.21s
{"msg":"announce","level":"INFO","ts":"2021-09-06T02:48:26.612401254-07:00","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","pid":"129599","version":"0.1.0","config":"Config { server_address: \"vsock://1321076924:1024\", bundle_dir: \"/tmp/bundle\", timeout_nano: 0, interactive: false, ignore_errors: false }"}
{"msg":"client setup complete","level":"INFO","ts":"2021-09-06T02:48:26.618215437-07:00","source":"kata-agent-ctl","subsystem":"rpc","pid":"129599","name":"kata-agent-ctl","version":"0.1.0","server-address":"vsock://1321076924:1024"}
{"msg":"Run command Check (1 of 1)","level":"INFO","ts":"2021-09-06T02:48:26.618286397-07:00","name":"kata-agent-ctl","version":"0.1.0","subsystem":"rpc","source":"kata-agent-ctl","pid":"129599"}
{"msg":"response received","level":"INFO","ts":"2021-09-06T02:48:26.619212840-07:00","version":"0.1.0","subsystem":"rpc","pid":"129599","source":"kata-agent-ctl","name":"kata-agent-ctl","response":"status: SERVING"}
{"msg":"Command Check (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-06T02:48:26.619281890-07:00","subsystem":"rpc","name":"kata-agent-ctl","pid":"129599","source":"kata-agent-ctl","version":"0.1.0"}
{"msg":"Run command GetGuestDetails (1 of 1)","level":"INFO","ts":"2021-09-06T02:48:26.619328342-07:00","pid":"129599","version":"0.1.0","subsystem":"rpc","source":"kata-agent-ctl","name":"kata-agent-ctl"}
{"msg":"response received","level":"INFO","ts":"2021-09-06T02:48:26.622968404-07:00","pid":"129599","version":"0.1.0","subsystem":"rpc","name":"kata-agent-ctl","source":"kata-agent-ctl","response":"mem_block_size_bytes: 134217728 agent_details {version: \"2.3.0-alpha0\" storage_handlers: \"blk\" storage_handlers: \"9p\" storage_handlers: \"virtio-fs\" storage_handlers: \"ephemeral\" storage_handlers: \"mmioblk\" storage_handlers: \"local\" storage_handlers: \"scsi\" storage_handlers: \"nvdimm\" storage_handlers: \"watchable-bind\"}"}
{"msg":"Command GetGuestDetails (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-06T02:48:26.623049042-07:00","name":"kata-agent-ctl","pid":"129599","source":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0"}
{"msg":"Run command PullImage (1 of 1)","level":"INFO","ts":"2021-09-06T02:48:26.623081584-07:00","subsystem":"rpc","pid":"129599","name":"kata-agent-ctl","source":"kata-agent-ctl","version":"0.1.0"}
{"msg":"response received","level":"INFO","ts":"2021-09-06T02:48:54.270118679-07:00","subsystem":"rpc","version":"0.1.0","source":"kata-agent-ctl","name":"kata-agent-ctl","pid":"129599","response":""}
{"msg":"Command PullImage (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-06T02:48:54.270228983-07:00","pid":"129599","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0"}
```
- In the kata shell terminal you can see the container bundle has been created:
```
$ ls -al /run/kata-containers/01234567889
total 1216
drwx------  3 root root     120 Sep  6 09:48 .
drwxr-xr-x  7 root root     140 Sep  6 09:48 ..
-rw-r--r--  1 root root    3088 Sep  6 09:48 config.json
dr-xr-xr-x 18 root root     440 Aug  9 05:48 rootfs
-rw-r--r--  1 root root 1235681 Sep  6 09:48 sha256_6db7cf62a51ac7d5b573f7a61a855093ff82d7c1caaf1413e7b4730a20a172d0.mtree
-rw-r--r--  1 root root     372 Sep  6 09:48 umoci.json
```
- The console shell shows what has happened:
```
{"msg":"get guest details!","level":"INFO","ts":"2021-09-06T09:48:26.561831537+00:00","subsystem":"rpc","pid":"56","name":"kata-agent","source":"agent","version":"0.1.0"}
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
   • unpack rootfs: /run/kata-containers/01234567889/rootfs
   • unpack layer: sha256:ecfb9899f4ce3412a027b88f47dfea56664b5d4bc35eaa0f12c94c671f8ba503
   • ... done
   • computing filesystem manifest ...
   • ... done
   • unpacked image bundle: /run/kata-containers/01234567889
{"msg":"cid is \"01234567889\"","level":"INFO","ts":"2021-09-06T09:48:42.738268945+00:00","source":"agent","name":"kata-agent","subsystem":"rpc","pid":"56","version":"0.1.0"}
{"msg":"target_path_bundle is \"/run/kata-containers/01234567889\"","level":"INFO","ts":"2021-09-06T09:48:42.738355998+00:00","name":"kata-agent","source":"agent","pid":"56","subsystem":"rpc","version":"0.1.0"}
{"msg":"handling signal","level":"INFO","ts":"2021-09-06T09:48:54.212793601+00:00","name":"kata-agent","version":"0.1.0","pid":"56","subsystem":"signals","source":"agent","signal":"SIGCHLD"}
```

## Additional script usage

As well as being able to use the script as above to build all of kata-containers from scratch it can be used to just re-build bits of it by running the script with different parameters. For example after the first build you will often not need to re-install the dependencies, QEMU or the Guest kernel, but just test code changes made to the runtime and agent. This can be done by running `. ~/ccv0.sh -d rebuild_and_install_kata` (*Note this re-does the checkout from git, do you changes are only made locally it is better to do the individual steps e.g. `. ~/ccv0.sh -d build_kata_runtime && . ~/ccv0.sh -d build_and_add_agent_to_rootfs && . ~/ccv0.sh -d build_and_install_rootfs`).* There are commands for a lot of steps in building, setting up and testing and the full list can be seen by running `~/ccv0.sh help`:
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
- agent_list_commands:          List agent commands on agent-ctl
- test:                         Test using kata with containerd
- test_capture_logs:            Test using kata with containerd and capture the logs in the user's home directory

Options:
    -d: Enable debug
    -h: Display this help
```