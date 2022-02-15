# How to build, run and test Kata CCv0

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
    - By default `ccv0.sh` enables the agent to use the rust implementation to pull container images on the guest. If
      you wish to instead build and include the `skopeo` package for this then run
      ```bash
      $ export SKOPEO=yes
      ```
      `skopeo` is
      required for passing source credentials and verifying container image signatures using the kata agent.

### Using `crictl` to do end-to-end testing of provisioning a container with the unencrypted image pulled on the guest

- Run the full build process with Kubernetes off, so it's configure doesn't interfere with `crictl` using:
  ```bash
  $ export KUBERNETES="no"
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
          enabling console access to the kata-runtime, which should only be done in development)
        - Create, build and install a rootfs for the Kata hypervisor to use. For 'CCv0' this is currently based on Ubuntu
        20.04 and has extra packages like `umoci` added.
        - Build the Kata guest kernel
        - Install QEMU
    > **Note**: Depending on how where your VMs are hosted and how IPs are shared you might get an error from docker
    during matching `ERROR: toomanyrequests: Too Many Requests`. To get past
    this, login into Docker Hub and pull the images used with:
  >  ```bash
  >  $ docker login
  >  $ sudo docker pull ubuntu
  >  ```
  >  then re-run the command.
    - The first time this runs it may take a while, but subsequent runs will be quicker as more things are already
      installed and they can be further cut down by not running all the above steps 
      [see "Additional script usage" below](#additional-script-usage)
   
- Create a new kata sandbox pod using `crictl` with:
  ```bash
  $ ~/ccv0.sh crictl_create_cc_pod
  ```
    - This creates a pod configuration file, creates the pod from this using 
    `sudo crictl runp -r kata ~/pod-config.yaml` and runs `sudo crictl pods` to show the pod
- Create a new kata confidential container with:
  ```bash
  $ ~/ccv0.sh crictl_create_cc_container
  ```
    - This creates a container (based on `busybox:1.33.1`) in the kata cc sandbox and prints a list of containers.
      This will have been created based on an image pulled in the kata pod sandbox/guest, not on the host machine.

### Validate that the container image was pulled on the guest

There are a couple of ways we can check that the container pull image action was offloaded to the guest, by checking
the guest's file system for the unpacked bundle and checking the host's directories to ensure it wasn't also pulled
there.
- To check the guest's file system:
    - Open a shell into the sandbox with:
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
        -rw-r--r--  1 root root   372 Jan 20 10:03 umoci.json
        -rw-r--r--  1 root root 63584 Jan 20 10:03 sha256_be9faa75035c20288cde7d2cdeb6cd1f5f4dbcd845d3f86f7feab61c4eff9eb5.mtree
        drwxr-xr-x 12 root root   240 Jan 20 10:03 rootfs
        ```
        which shows how the image has been pulled and then unbundled on the guest.
    - Leave the kata shell by running:
      ```bash
      $ exit
      ```
- To verify that the image wasn't pulled on the host system we can look at the shared sandbox on the host and we
  should only see a single bundle for the pause container as the `busybox` based container image should have been
  pulled on the guest:
    - Find all the `rootfs` directories under in the pod's shared directory with:
      ```bash
      $ pod_id=$(ps -ef | grep qemu | egrep -o "sandbox-[^,][^,]*" | sed 's/sandbox-//g' | awk '{print $1}')
      $ sudo find /run/kata-containers/shared/sandboxes/${pod_id}/shared -name rootfs
      ./e89596e9de45ef2a154a5164554c9816293ab757cfd7a53d593fa144192a9964/rootfs
      ```
      which should only show a single `rootfs` directory if the container image was pulled on the guest, not the host
    - Looking that `rootfs` directory with
      ```bash
      $ sudo ls -ltr $(sudo find /run/kata-containers/shared/sandboxes/${pod_id}/shared -name rootfs)
      ```
      prints something similar to
      ```
      total 668
      -rwxr-xr-x 1 root root 682696 Aug 25 13:58 pause
      drwxr-xr-x 2 root root      6 Jan 20 02:01 proc
      drwxr-xr-x 2 root root      6 Jan 20 02:01 dev
      drwxr-xr-x 2 root root      6 Jan 20 02:01 sys
      drwxr-xr-x 2 root root     25 Jan 20 02:01 etc
      ```
      which is clearly the pause container indicating that the `busybox` based container image if not exposed to the host.

#### Clean up `crictl` pod sandbox and container
- When the testing is complete you can either continue on with different tests (mentioned below) using the pod sandbox, or delete the container and pod by running:
  ```bash
  $ ~/ccv0.sh crictl_delete_cc
  ```

### Setting up Kubernetes

The documentation for end-to-end testing of a confidential container created through Kubernetes 
[is not completed yet](https://github.com/kata-containers/kata-containers/issues/3511),
but Kubernetes can be used to create a non-confidential kata pod using `ccv0.sh`.

- Run the full build process with the Kubernetes environment variable set to `"yes"`, so the Kubernetes cluster is configured and created using the VM
  as a single node cluster: 
  ```bash
  $ export KUBERNETES="yes"
  $ ~/ccv0.sh build_and_install_all
  ```
    > **Note**: Depending on how where your VMs are hosted and how IPs are shared you might get an error from docker
    during matching `ERROR: toomanyrequests: Too Many Requests`. To get past
    this, login into Docker Hub and pull the images used with:
  >  ```bash
  >  $ docker login
  >  $ sudo docker pull registry:2
  >  $ sudo docker pull nginx
  >  $ sudo docker pull ubuntu
  >  ```
  >  then re-run the command.
- Check that your Kubernetes cluster has been correctly set-up: 
```
$ kubectl get nodes
NAME                              STATUS   ROLES                  AGE     VERSION
stevenh-ccv0-demo1.fyre.ibm.com   Ready    control-plane,master   3m33s   v1.21.1
```
- Create a kata pod:
```
$ ~/ccv0.sh create_kata_pod
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
- This Kubernetes pod can now be used for further testing (mentioned below) using the created kata pod sandbox, or deleted
  by running
  ```bash
  $ ~/ccv0.sh delete_kata_pod
  ```


### Using a kata pod sandbox for testing with `agent-ctl` or `ctr shim`

Once you have a kata pod sandbox created as described above, either using 
[`crictl`](#using-crictl-to-do-end-to-end-testing-of-provisioning-a-container-with-the-unencrypted-image-pulled-on-the-guest)
or [Kubernetes](#setting-up-kubernetes), you can use this to test specific components of the kata confidential
containers architecture. This can be useful for development and debugging to isolate and test features
that aren't broadly supported end-to-end. Here are some examples:

- For debugging purposed you can optionally create a new terminal on the VM and connect to the kata guest's console log:
  ```bash
  $ ~/ccv0.sh open_kata_console
  ```
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
- Alternatively you can issue the command directly to the kata-agent pull image endpoint, which also supports
  credentials in order to pull from an authenticated registry:
    - Optionally set up some environment variables to set the image and credentials used:
        - Set the `PULL_IMAGE` environment variable e.g. `export PULL_IMAGE="docker.io/library/busybox:latest"`
          if a specific container image is required.
        - If the container registry for the image requires authentication then this can be set with an environment
          variable `SOURCE_CREDS`. For example to use Docker Hub (`docker.io`) as an authenticated user first run 
          `export SOURCE_CREDS="<dockerhub username>:<dockerhub api key>"`
            > **Note**: the credentials support on the agent request is a tactical solution for the short-term
              proof of concept to allow more images to be pulled and tested. Once we have support for getting
              keys into the kata guest using the attestation-agent and/or KBS I'd expect container registry
              credentials to be looked up using that mechanism.
            
            > **Note**: the native rust implementation doesn't current flow credentials at the moment, so use
            the `skopeo` based implementation if they are needed now.
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
- To validate that the image pull was successful, you can open a shell into the kata pod with:
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
  -rw-r--r--  1 root root   372 Jan 20 16:45 umoci.json
  -rw-r--r--  1 root root 63584 Jan 20 16:45 sha256_be9faa75035c20288cde7d2cdeb6cd1f5f4dbcd845d3f86f7feab61c4eff9eb5.mtree
  ```
- Leave the kata shell by running:
  ```bash
  $ exit
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
- If you don't already have a kata pod sandbox created, follow the instructions above to create one either using
 [`crictl`](#using-crictl-to-do-end-to-end-testing-of-provisioning-a-container-with-the-unencrypted-image-pulled-on-the-guest)
  or [Kubernetes](#setting-up-kubernetes)
- To test the fallback behaviour works using an unsigned image on an *unprotected* registry we can pull the `busybox`
image by running:
  ```bash
  $ export CONTAINER_ID="unprotected-unsigned"
  $ export PULL_IMAGE="docker.io/library/busybox:latest"
  $ ~/ccv0.sh agent_pull_image
  ```
  - This finishes with a return `Ok()`
- To test that an unsigned image from our *protected* test container registry is rejected we can run:
  ```bash
  $ export CONTAINER_ID="protected-unsigned"
  $ export PULL_IMAGE="quay.io/kata-containers/confidential-containers:unsigned"
  $ ~/ccv0.sh agent_pull_image
  ```
  - This results in an `ERROR: API failed` message from `agent_ctl` and the Kata sandbox console log shows the correct
  cause that the signature we has was not valid for the unsigned image:
  ```text
  FATA[0001] Source image rejected: Signature for identity quay.io/kata-containers/confidential-containers:signed is not accepted
  ```
- To test that the signed image our *protected* test container registry is accepted we can run:
  ```bash
  $ export CONTAINER_ID="protected-signed"
  $ export PULL_IMAGE="quay.io/kata-containers/confidential-containers:signed"
  $ ~/ccv0.sh agent_pull_image
  ```
  - This finishes with a return `Ok()`
- Finally to check the image with a valid signature, but invalid GPG key (the real trusted piece of information we really
want to protect with the attestation agent in future) fails we can run: 
  ```bash
  $ export CONTAINER_ID="protected-wrong-key"
  $ export PULL_IMAGE="quay.io/kata-containers/confidential-containers:other_signed"
  $ ~/ccv0.sh agent_pull_image
  ```
  - Again this results in an `ERROR: API failed` message from `agent_ctl` and the Kata sandbox console log shows a
  slightly different error:
  ```text
  FATA[0001] Source image rejected: Invalid GPG signature...

  ```
- To confirm that the first and third tests create the image bundles correct we can open a shell into the kata pod with:
  ```bash
  $ ~/ccv0.sh open_kata_shell
  ```
- In the pod we can check the directories the images bundles were unpacked to:
  ```bash
  $ ls -ltr /run/kata-containers/unprotected-unsigned/
  ```
  should show something like
  ```
  total 72
  drwxr-xr-x 10 root root   200 Jan  1  1970 rootfs
  -rw-r--r--  1 root root  2977 Jan 26 16:06 config.json
  -rw-r--r--  1 root root   372 Jan 26 16:06 umoci.json
  -rw-r--r--  1 root root 63724 Jan 26 16:06 sha256_1612e16ff3f6b0d09eefdc4e9d5c5c0624f63032743e016585b095b958778016.mtree
  ```
  and
  ```bash
  $ ls -ltr /run/kata-containers/protected-signed/
  ```
  should show something like
  ```
  total 72
  drwxr-xr-x 10 root root   200 Jan  1  1970 rootfs
  -rw-r--r--  1 root root  2977 Jan 26 16:07 config.json
  -rw-r--r--  1 root root   372 Jan 26 16:07 umoci.json
  -rw-r--r--  1 root root 63568 Jan 26 16:07 sha256_ebf391d3f0ba36d4b64999ebbeadc878d229faec8839254a1c2264cf47735841.mtree
  ```

## Additional script usage

As well as being able to use the script as above to build all of `kata-containers` from scratch it can be used to just
re-build bits of it by running the script with different parameters. For example after the first build you will often
not need to re-install the dependencies, QEMU or the Guest kernel, but just test code changes made to the runtime and
agent. This can be done by running `~/ccv0.sh rebuild_and_install_kata`. (*Note this does a hard checkout*
*from git, so if your changes are only made locally it is better to do the individual steps e.g.* 
`~/ccv0.sh build_kata_runtime && ~/ccv0.sh build_and_add_agent_to_rootfs && ~/ccv0.sh build_and_install_rootfs`).
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
- initialize:                   Install dependencies and check out kata-containers source
- rebuild_and_install_kata:     Rebuild the kata runtime and agent and build and install the image
- build_kata_runtime:           Build and install the kata runtime
- configure:                    Configure Kata to use rootfs and enable debug
- create_rootfs:                Create a local rootfs
- build_and_add_agent_to_rootfs:Builds the kata-agent and adds it to the rootfs
- build_and_install_rootfs:     Builds and installs the rootfs image
- install_guest_kernel:         Setup, build and install the guest kernel
- build_qemu:                   Checkout, patch, build and install QEMU
- init_kubernetes:              initialize a Kubernetes cluster on this system
- crictl_create_cc_pod          Use crictl to create a new kata cc pod
- crictl_create_cc_container    Use crictl to create a new busybox container in the kata cc pod
- crictl_delete_cc              Use crictl to delete the kata cc pod sandbox and container in it
- create_kata_pod:              Create a kata runtime nginx pod in Kubernetes
- delete_kata_pod:              Delete a kata runtime nginx pod in Kubernetes
- restart_kata_pod:             Delete the kata nginx pod, then re-create it
- open_kata_console:            Stream the kata runtime's console
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
