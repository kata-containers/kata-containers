# How to build, run and test Kata CCv0 (archived)

## Introduction and Background

In order to try and make building (locally) and demoing the Kata Containers `CCv0` code base as simple as possible I've
shared a script [`ccv0.sh`](./ccv0.sh). This script was originally my attempt to automate the steps of the
[Developer Guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md) so that I could do
different sections of them repeatedly and reliably as I was playing around with make changes to different parts of the
Kata code base. I then tried to weave in some of the [`tests/.ci`](https://github.com/kata-containers/tests/tree/main/.ci)
scripts in order to have less duplicated code.
As we're progress on the confidential containers journey I hope to add more features to demonstrate the functionality
we have working.

*Disclaimer: This script has mostly just been used and tested by me ([@stevenhorsman](https://github.com/stevenhorsman)),*
*so there might be issues with it. I'm happy to try and help solve these if possible, but this shouldn't be considered a*
*fully supported process by the Kata Containers community.*

### Basic script set-up and optional environment variables

In order to build, configure and demo the CCv0 functionality, these are the set-up steps I take:
- Provision a new VM
    - *I choose a Ubuntu 20.04 8GB VM for this as I had one available. There are some dependences on apt-get installed*
    *packages, so these will need re-working to be compatible with other platforms.*
- Copy the script over to your VM *(I put it in the home directory)* and ensure it has execute permission by running
```bash
$ chmod u+x ccv0.sh
```
- Optionally set up some environment variables
    - By default the script checks out the `CCv0` branches of the `kata-containers/kata-containers` and
      `kata-containers/tests` repositories, but it is designed to be used to test of personal forks and branches as well.
      If you want to build and run these you can export the `katacontainers_repo`, `katacontainers_branch`, `tests_repo`
      and `tests_branch` variables e.g.
      ```bash
      $ export katacontainers_repo=github.com/stevenhorsman/kata-containers
      $ export katacontainers_branch=stevenh/agent-pull-image-endpoint
      $ export tests_repo=github.com/stevenhorsman/tests
      $ export tests_branch=stevenh/add-ccv0-changes-to-build
      ```
      before running the script.
    - By default the build and configuration are using `QEMU` as the hypervisor. In order to use `Cloud Hypervisor` instead
      set:
      ```
      $ export KATA_HYPERVISOR="cloud-hypervisor"
      ```
      before running the build.

- At this point you can provision a Kata confidential containers pod and container with either
  [`crictl`](#using-crictl-for-end-to-end-provisioning-of-a-kata-confidential-containers-pod-with-an-unencrypted-image),
  or [Kubernetes](#using-kubernetes-for-end-to-end-provisioning-of-a-kata-confidential-containers-pod-with-an-unencrypted-image)
  and then test and use it.

### Using crictl for end-to-end provisioning of a Kata confidential containers pod with an unencrypted image

- Run the full build process with Kubernetes turned off, so its configuration doesn't interfere with `crictl` using:
  ```bash
  $ export KUBERNETES="no"
  $ export KATA_HYPERVISOR="qemu"
  $ ~/ccv0.sh -d build_and_install_all
  ```
    > **Note**: Much of this script has to be run as `sudo`, so you are likely to get prompted for your password.
    - *I run this script sourced just so that the required installed components are accessible on the `PATH` to the rest*
      *of the process without having to reload the session.*
    - The steps that `build_and_install_all` takes is:
        - Checkout the git repos for the `tests` and `kata-containers` repos as specified by the environment variables
        (default to `CCv0` branches if they are not supplied)
        - Use the `tests/.ci` scripts to install the build dependencies
        - Build and install the Kata runtime
        - Configure Kata to use containerd and for debug and confidential containers features to be enabled (including
          enabling console access to the Kata guest shell, which should only be done in development)
        - Create, build and install a rootfs for the Kata hypervisor to use. For 'CCv0' this is currently based on Ubuntu
        20.04.
        - Build the Kata guest kernel
        - Install the hypervisor (in order to select which hypervisor will be used, the `KATA_HYPERVISOR` environment
        variable can be used to select between `qemu` or `cloud-hypervisor`)
    > **Note**: Depending on how where your VMs are hosted and how IPs are shared you might get an error from docker
    during matching `ERROR: toomanyrequests: Too Many Requests`. To get past
    this, login into Docker Hub and pull the images used with:
  >  ```bash
  >  $ sudo docker login
  >  $ sudo docker pull ubuntu
  >  ```
  >  then re-run the command.
    - The first time this runs it may take a while, but subsequent runs will be quicker as more things are already
      installed and they can be further cut down by not running all the above steps
      [see "Additional script usage" below](#additional-script-usage)

- Create a new Kata sandbox pod using `crictl` with:
  ```bash
  $ ~/ccv0.sh crictl_create_cc_pod
  ```
    - This creates a pod configuration file, creates the pod from this using
    `sudo crictl runp -r kata ~/pod-config.yaml` and runs `sudo crictl pods` to show the pod
- Create a new Kata confidential container with:
  ```bash
  $ ~/ccv0.sh crictl_create_cc_container
  ```
    - This creates a container (based on `busybox:1.33.1`) in the Kata cc sandbox and prints a list of containers.
      This will have been created based on an image pulled in the Kata pod sandbox/guest, not on the host machine.

As this point you should have a `crictl` pod and container that is using the Kata confidential containers runtime.
You can [validate that the container image was pulled on the guest](#validate-that-the-container-image-was-pulled-on-the-guest)
or [using the Kata pod sandbox for testing with `agent-ctl` or `ctr shim`](#using-a-kata-pod-sandbox-for-testing-with-agent-ctl-or-ctr-shim)

#### Clean up the `crictl` pod sandbox and container
- When the testing is complete you can delete the container and pod by running:
  ```bash
  $ ~/ccv0.sh crictl_delete_cc
  ```
### Using Kubernetes for end-to-end provisioning of a Kata confidential containers pod with an unencrypted image

- Run the full build process with the Kubernetes environment variable set to `"yes"`, so the Kubernetes cluster is
  configured and created using the VM
  as a single node cluster:
  ```bash
  $ export KUBERNETES="yes"
  $ ~/ccv0.sh build_and_install_all
  ```
    > **Note**: Depending on how where your VMs are hosted and how IPs are shared you might get an error from docker
    during matching `ERROR: toomanyrequests: Too Many Requests`. To get past
    this, login into Docker Hub and pull the images used with:
  >  ```bash
  >  $ sudo docker login
  >  $ sudo docker pull registry:2
  >  $ sudo docker pull ubuntu:20.04
  >  ```
  >  then re-run the command.
- Check that your Kubernetes cluster has been correctly set-up by running :
  ```bash
  $ kubectl get nodes
  ```
  and checking that you see a single node e.g.
  ```text
  NAME                             STATUS   ROLES                  AGE   VERSION
  stevenh-ccv0-k8s1.fyre.ibm.com   Ready    control-plane,master   43s   v1.22.0
  ```
- Create a Kata confidential containers pod by running:
  ```bash
  $ ~/ccv0.sh kubernetes_create_cc_pod
  ```
- Wait a few seconds for pod to start then check that the pod's status is `Running` with
  ```bash
  $ kubectl get pods
  ```
  which should show something like:
  ```text
  NAME         READY   STATUS    RESTARTS   AGE
  busybox-cc   1/1     Running   0          54s
  ```

- As this point you should have a Kubernetes pod and container running, that is using the Kata
confidential containers runtime.
You can [validate that the container image was pulled on the guest](#validate-that-the-container-image-was-pulled-on-the-guest)
or [using the Kata pod sandbox for testing with `agent-ctl` or `ctr shim`](#using-a-kata-pod-sandbox-for-testing-with-agent-ctl-or-ctr-shim)

#### Clean up the Kubernetes pod sandbox and container
- When the testing is complete you can delete the container and pod by running:
  ```bash
  $ ~/ccv0.sh kubernetes_delete_cc_pod
  ```

### Validate that the container image was pulled on the guest

There are a couple of ways we can check that the container pull image action was offloaded to the guest, by checking
the guest's file system for the unpacked bundle and checking the host's directories to ensure it wasn't also pulled
there.
- To check the guest's file system:
    - Open a shell into the Kata guest with:
      ```bash
      $ ~/ccv0.sh open_kata_shell
      ```
    - List the files in the directory that the container image bundle should have been unpacked to with:
      ```bash
      $ ls -ltr /run/kata-containers/confidential-containers_signed/
      ```
    - This should give something like
        ```
        total 72
        -rw-r--r--  1 root root  2977 Jan 20 10:03 config.json
        drwxr-xr-x 12 root root   240 Jan 20 10:03 rootfs
        ```
        which shows how the image has been pulled and then unbundled on the guest.
    - Leave the Kata guest shell by running:
      ```bash
      $ exit
      ```
- To verify that the image wasn't pulled on the host system we can look at the shared sandbox on the host and we
  should only see a single bundle for the pause container as the `busybox` based container image should have been
  pulled on the guest:
    - Find all the `rootfs` directories under in the pod's shared directory with:
      ```bash
      $ pod_id=$(ps -ef | grep containerd-shim-kata-v2 | egrep -o "id [^,][^,].* " | awk '{print $2}')
      $ sudo find /run/kata-containers/shared/sandboxes/${pod_id}/shared -name rootfs
      ```
      which should only show a single `rootfs` directory if the container image was pulled on the guest, not the host
    - Looking that `rootfs` directory with
      ```bash
      $ sudo ls -ltr $(sudo find /run/kata-containers/shared/sandboxes/${pod_id}/shared -name rootfs)
      ```
      shows something similar to
      ```
      total 668
      -rwxr-xr-x 1 root root 682696 Aug 25 13:58 pause
      drwxr-xr-x 2 root root      6 Jan 20 02:01 proc
      drwxr-xr-x 2 root root      6 Jan 20 02:01 dev
      drwxr-xr-x 2 root root      6 Jan 20 02:01 sys
      drwxr-xr-x 2 root root     25 Jan 20 02:01 etc
      ```
      which is clearly the pause container indicating that the `busybox` based container image is not exposed to the host.

### Using a Kata pod sandbox for testing with `agent-ctl` or `ctr shim`

Once you have a kata pod sandbox created as described above, either using
[`crictl`](#using-crictl-for-end-to-end-provisioning-of-a-kata-confidential-containers-pod-with-an-unencrypted-image), or [Kubernetes](#using-kubernetes-for-end-to-end-provisioning-of-a-kata-confidential-containers-pod-with-an-unencrypted-image)
, you can use this to test specific components of the Kata confidential
containers architecture. This can be useful for development and debugging to isolate and test features
that aren't broadly supported end-to-end. Here are some examples:

- In the first terminal run the pull image on guest command against the Kata agent, via the shim (`containerd-shim-kata-v2`).
This can be achieved using the [containerd](https://github.com/containerd/containerd) CLI tool, `ctr`, which can be used to
interact with the shim directly. The command takes the form
`ctr --namespace k8s.io shim --id <sandbox-id> pull-image <image> <new-container-id>` and can been run directly, or through
the `ccv0.sh` script to automatically fill in the variables:
  - Optionally, set up some environment variables to set the image and credentials used:
    - By default the shim pull image test in `ccv0.sh` will use the `busybox:1.33.1` based test image
     `quay.io/kata-containers/confidential-containers:signed` which requires no authentication. To use a different
      image, set the `PULL_IMAGE` environment variable e.g.
      ```bash
      $ export PULL_IMAGE="docker.io/library/busybox:latest"
      ```
      Currently the containerd shim pull image
      code doesn't support using a container registry that requires authentication, so if this is required, see the
      below steps to run the pull image command against the agent directly.
  - Run the pull image agent endpoint with:
    ```bash
    $ ~/ccv0.sh shim_pull_image
    ```
    which we print the `ctr shim` command for reference
- Alternatively you can issue the command directly to the `kata-agent` pull image endpoint, which also supports
  credentials in order to pull from an authenticated registry:
    - Optionally set up some environment variables to set the image and credentials used:
        - Set the `PULL_IMAGE` environment variable e.g. `export PULL_IMAGE="docker.io/library/busybox:latest"`
          if a specific container image is required.
        - If the container registry for the image requires authentication then this can be set with an environment
          variable `SOURCE_CREDS`. For example to use Docker Hub (`docker.io`) as an authenticated user first run
          `export SOURCE_CREDS="<dockerhub username>:<dockerhub api key>"`
            > **Note**: the credentials support on the agent request is a tactical solution for the short-term
              proof of concept to allow more images to be pulled and tested. Once we have support for getting
              keys into the Kata guest image using the attestation-agent and/or KBS I'd expect container registry
              credentials to be looked up using that mechanism.
    - Run the pull image agent endpoint with
      ```bash
      $ ~/ccv0.sh agent_pull_image
      ```
      and you should see output which includes `Command PullImage (1 of 1) returned (Ok(()), false)` to indicate
      that the `PullImage` request was successful e.g.
      ```
      Finished release [optimized] target(s) in 0.21s
      {"msg":"announce","level":"INFO","ts":"2021-09-15T08:40:14.189360410-07:00","subsystem":"rpc","name":"kata-agent-ctl","pid":"830920","version":"0.1.0","source":"kata-agent-ctl","config":"Config { server_address: \"vsock://1970354082:1024\", bundle_dir: \"/tmp/bundle\", timeout_nano: 0, interactive: false, ignore_errors: false }"}
      {"msg":"client setup complete","level":"INFO","ts":"2021-09-15T08:40:14.193639057-07:00","pid":"830920","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0","server-address":"vsock://1970354082:1024"}
      {"msg":"Run command PullImage (1 of 1)","level":"INFO","ts":"2021-09-15T08:40:14.196643765-07:00","pid":"830920","source":"kata-agent-ctl","subsystem":"rpc","name":"kata-agent-ctl","version":"0.1.0"}
      {"msg":"response received","level":"INFO","ts":"2021-09-15T08:40:43.828200633-07:00","source":"kata-agent-ctl","name":"kata-agent-ctl","subsystem":"rpc","version":"0.1.0","pid":"830920","response":""}
      {"msg":"Command PullImage (1 of 1) returned (Ok(()), false)","level":"INFO","ts":"2021-09-15T08:40:43.828261708-07:00","subsystem":"rpc","pid":"830920","source":"kata-agent-ctl","version":"0.1.0","name":"kata-agent-ctl"}
      ```
      > **Note**: The first time that `~/ccv0.sh agent_pull_image` is run, the `agent-ctl` tool will be built
      which may take a few minutes.
- To validate that the image pull was successful, you can open a shell into the Kata guest with:
  ```bash
  $ ~/ccv0.sh open_kata_shell
  ```
- Check the `/run/kata-containers/` directory to verify that the container image bundle has been created in a directory
  named either `01234556789` (for the container id), or the container image name, e.g.
  ```bash
  $ ls -ltr /run/kata-containers/confidential-containers_signed/
  ```
  which should show something like
  ```
  total 72
  drwxr-xr-x 10 root root   200 Jan  1  1970 rootfs
  -rw-r--r--  1 root root  2977 Jan 20 16:45 config.json
  ```
- Leave the Kata shell by running:
  ```bash
  $ exit
  ```

## Verifying signed images

For this sample demo, we use local attestation to pass through the required
configuration to do container image signature verification. Due to this, the ability to verify images is limited
to a pre-created selection of test images in our test
repository [`quay.io/kata-containers/confidential-containers`](https://quay.io/repository/kata-containers/confidential-containers?tab=tags).
For pulling images not in this test repository (called an *unprotected* registry below), we fall back to the behaviour
of not enforcing signatures. More documentation on how to customise this to match your own containers through local,
or remote attestation will be available in future.

In our test repository there are three tagged images:

| Test Image | Base Image used | Signature status | GPG key status |
| --- | --- | --- | --- |
| `quay.io/kata-containers/confidential-containers:signed` | `busybox:1.33.1` | [signature](https://github.com/kata-containers/tests/tree/CCv0/integration/confidential/fixtures/quay_verification/x86_64/signatures.tar) embedded in kata rootfs |  [public key](https://github.com/kata-containers/tests/tree/CCv0/integration/confidential/fixtures/quay_verification/x86_64/public.gpg) embedded in kata rootfs |
| `quay.io/kata-containers/confidential-containers:unsigned` | `busybox:1.33.1` | not signed | not signed |
| `quay.io/kata-containers/confidential-containers:other_signed` | `nginx:1.21.3` | [signature](https://github.com/kata-containers/tests/tree/CCv0/integration/confidential/fixtures/quay_verification/x86_64/signatures.tar) embedded in kata rootfs | GPG key not kept |

Using a standard unsigned `busybox` image that can be pulled from another, *unprotected*, `quay.io` repository we can
test a few scenarios.

In this sample, with local attestation, we pass in the the public GPG key and signature files, and the [`offline_fs_kbc`
configuration](https://github.com/confidential-containers/attestation-agent/blob/main/src/kbc_modules/offline_fs_kbc/README.md)
into the guest image which specifies that any container image from `quay.io/kata-containers`
must be signed with the embedded GPG key and the agent configuration needs updating to enable this.
With this policy set a few tests of image verification can be done to test different scenarios by attempting
to create containers from these images using `crictl`:

- If you don't already have the Kata Containers CC code built and configured for `crictl`, then follow the
[instructions above](#using-crictl-for-end-to-end-provisioning-of-a-kata-confidential-containers-pod-with-an-unencrypted-image)
up to the `~/ccv0.sh crictl_create_cc_pod` command.

- In order to enable the guest image, you will need to setup the required configuration, policy and signature files
needed by running
`~/ccv0.sh copy_signature_files_to_guest` and then run `~/ccv0.sh crictl_create_cc_pod` which will delete and recreate
your pod - adding in the new files.

- To test the fallback behaviour works using an unsigned image from an *unprotected* registry we can pull the `busybox`
image by running:
  ```bash
  $ export CONTAINER_CONFIG_FILE=container-config_unsigned-unprotected.yaml
  $ ~/ccv0.sh crictl_create_cc_container
  ```
  - This finishes showing the running container e.g.
  ```text
  CONTAINER           IMAGE                               CREATED                  STATE               NAME                        ATTEMPT             POD ID
  98c70fefe997a       quay.io/prometheus/busybox:latest   Less than a second ago   Running             prometheus-busybox-signed   0                   70119e0539238
  ```
- To test that an unsigned image from our *protected* test container registry is rejected we can run:
  ```bash
  $ export CONTAINER_CONFIG_FILE=container-config_unsigned-protected.yaml
  $ ~/ccv0.sh crictl_create_cc_container
  ```
  - This correctly results in an error message from `crictl`:
  `PullImage from image service failed" err="rpc error: code = Internal desc = Security validate failed: Validate image failed: The signatures do not satisfied! Reject reason: [Match reference failed.]" image="quay.io/kata-containers/confidential-containers:unsigned"`
- To test that the signed image our *protected* test container registry is accepted we can run:
  ```bash
  $ export CONTAINER_CONFIG_FILE=container-config.yaml
  $ ~/ccv0.sh crictl_create_cc_container
  ```
  - This finishes by showing a new `kata-cc-busybox-signed` running container e.g.
  ```text
  CONTAINER           IMAGE                                                    CREATED                  STATE               NAME                        ATTEMPT             POD ID
  b4d85c2132ed9       quay.io/kata-containers/confidential-containers:signed   Less than a second ago   Running             kata-cc-busybox-signed      0                   70119e0539238
  ...
  ```
- Finally to check the image with a valid signature, but invalid GPG key (the real trusted piece of information we really
want to protect with the attestation agent in future) fails we can run:
  ```bash
  $ export CONTAINER_CONFIG_FILE=container-config_signed-protected-other.yaml
  $ ~/ccv0.sh crictl_create_cc_container
  ```
  - Again this results in an error message from `crictl`:
  `"PullImage from image service failed" err="rpc error: code = Internal desc = Security validate failed: Validate image failed: The signatures do not satisfied! Reject reason: [signature verify failed! There is no pubkey can verify the signature!]" image="quay.io/kata-containers/confidential-containers:other_signed"`

### Using Kubernetes to create a Kata confidential containers pod from the encrypted ssh demo sample image

The [ssh-demo](https://github.com/confidential-containers/documentation/tree/main/demos/ssh-demo) explains how to
demonstrate creating a Kata confidential containers pod from an encrypted image with the runtime created by the
[confidential-containers operator](https://github.com/confidential-containers/documentation/blob/main/demos/operator-demo).
To be fully confidential, this should be run on a Trusted Execution Environment, but it can be tested on generic
hardware as well.

If you wish to build the Kata confidential containers runtime to do this yourself, then you can using the following
steps:

- Run the full build process with the Kubernetes environment variable set to `"yes"`, so the Kubernetes cluster is
  configured and created using the VM as a single node cluster and with `AA_KBC` set to `offline_fs_kbc`.
  ```bash
  $ export KUBERNETES="yes"
  $ export AA_KBC=offline_fs_kbc
  $ ~/ccv0.sh build_and_install_all
  ```
    - The `AA_KBC=offline_fs_kbc` mode will ensure that, when creating the rootfs of the Kata guest, the
      [attestation-agent](https://github.com/confidential-containers/attestation-agent) will be added along with the
      [sample offline KBC](https://github.com/confidential-containers/documentation/blob/main/demos/ssh-demo/aa-offline_fs_kbc-keys.json)
      and an agent configuration file
    > **Note**: Depending on how where your VMs are hosted and how IPs are shared you might get an error from docker
    during matching `ERROR: toomanyrequests: Too Many Requests`. To get past
    this, login into Docker Hub and pull the images used with:
  >  ```bash
  >  $ sudo docker login
  >  $ sudo docker pull registry:2
  >  $ sudo docker pull ubuntu:20.04
  >  ```
  >  then re-run the command.
- Check that your Kubernetes cluster has been correctly set-up by running :
  ```bash
  $ kubectl get nodes
  ```
  and checking that you see a single node e.g.
  ```text
  NAME                             STATUS   ROLES                  AGE   VERSION
  stevenh-ccv0-k8s1.fyre.ibm.com   Ready    control-plane,master   43s   v1.22.0
  ```
- Create a sample Kata confidential containers ssh pod by running:
  ```bash
  $ ~/ccv0.sh kubernetes_create_ssh_demo_pod
  ```
- As this point you should have a Kubernetes pod running the Kata confidential containers runtime that has pulled
the [sample image](https://hub.docker.com/r/katadocker/ccv0-ssh) which was encrypted by the key file that we included
in the rootfs.
During the pod deployment the image was pulled and then decrypted using the key file, on the Kata guest image, without
it ever being available to the host.

- To validate that the container is working you, can connect to the image via SSH by running:
  ```bash
  $ ~/ccv0.sh connect_to_ssh_demo_pod
  ```
  - During this connection the host key fingerprint is shown and should match:
    `ED25519 key fingerprint is SHA256:wK7uOpqpYQczcgV00fGCh+X97sJL3f6G1Ku4rvlwtR0.`
  - After you are finished connecting then run:
    ```bash
    $ exit
    ```

- To delete the sample SSH demo pod run:
  ```bash
  $ ~/ccv0.sh kubernetes_delete_ssh_demo_pod
  ```

## Additional script usage

As well as being able to use the script as above to build all of `kata-containers` from scratch it can be used to just
re-build bits of it by running the script with different parameters. For example after the first build you will often
not need to re-install the dependencies, the hypervisor or the Guest kernel, but just test code changes made to the
runtime and agent. This can be done by running `~/ccv0.sh rebuild_and_install_kata`. (*Note this does a hard checkout*
*from git, so if your changes are only made locally it is better to do the individual steps e.g.*
`~/ccv0.sh build_kata_runtime && ~/ccv0.sh build_and_add_agent_to_rootfs && ~/ccv0.sh build_and_install_rootfs`).
There are commands for a lot of steps in building, setting up and testing and the full list can be seen by running
`~/ccv0.sh help`:
```
$ ~/ccv0.sh help
Overview:
    Build and test kata containers from source
    Optionally set kata-containers and tests repo and branch as exported variables before running
    e.g. export katacontainers_repo=github.com/stevenhorsman/kata-containers && export katacontainers_branch=kata-ci-from-fork && export tests_repo=github.com/stevenhorsman/tests && export tests_branch=kata-ci-from-fork && ~/ccv0.sh build_and_install_all
Usage:
    ccv0.sh [options] <command>
Commands:
- help:                         Display this help
- all:                          Build and install everything, test kata with containerd and capture the logs
- build_and_install_all:        Build and install everything
- initialize:                   Install dependencies and check out kata-containers source
- rebuild_and_install_kata:     Rebuild the kata runtime and agent and build and install the image
- build_kata_runtime:           Build and install the kata runtime
- configure:                    Configure Kata to use rootfs and enable debug
- create_rootfs:                Create a local rootfs
- build_and_add_agent_to_rootfs:Builds the kata-agent and adds it to the rootfs
- build_and_install_rootfs:     Builds and installs the rootfs image
- install_guest_kernel:         Setup, build and install the guest kernel
- build_cloud_hypervisor        Checkout, patch, build and install Cloud Hypervisor
- build_qemu:                   Checkout, patch, build and install QEMU
- init_kubernetes:              initialize a Kubernetes cluster on this system
- crictl_create_cc_pod          Use crictl to create a new kata cc pod
- crictl_create_cc_container    Use crictl to create a new busybox container in the kata cc pod
- crictl_delete_cc              Use crictl to delete the kata cc pod sandbox and container in it
- kubernetes_create_cc_pod:     Create a Kata CC runtime busybox-based pod in Kubernetes
- kubernetes_delete_cc_pod:     Delete the Kata CC runtime busybox-based pod in Kubernetes
- open_kata_shell:              Open a shell into the kata runtime
- agent_pull_image:             Run PullImage command against the agent with agent-ctl
- shim_pull_image:              Run PullImage command against the shim with ctr
- agent_create_container:       Run CreateContainer command against the agent with agent-ctl
- test:                         Test using kata with containerd
- test_capture_logs:            Test using kata with containerd and capture the logs in the user's home directory

Options:
    -d: Enable debug
    -h: Display this help
```
