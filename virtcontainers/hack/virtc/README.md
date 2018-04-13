# virtc

`virtc` is a simple command-line tool that serves to demonstrate typical usage of the virtcontainers API.
This is example software; unlike other projects like runc, runv, or rkt, virtcontainers is not a full container runtime.

## Virtc example

Here we explain how to use the sandbox and container API from `virtc` command line.

### Prepare your environment

#### Get your kernel

_Fedora_
```
$ sudo -E dnf config-manager --add-repo http://download.opensuse.org/repositories/home:clearlinux:preview:clear-containers-2.1/Fedora_25/home:clearlinux:preview:clear-containers-2.1.repo
$ sudo dnf install linux-container 
```

_Ubuntu_
```
$ sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/clearlinux:/preview:/clear-containers-2.1/xUbuntu_16.10/ /' >> /etc/apt/sources.list.d/cc-oci-runtime.list"
$ sudo apt install linux-container
```

#### Get your image

Retrieve a recent Clear Containers image to make sure it contains a recent version of hyperstart agent.

To download and install the latest image:

```
$ latest_version=$(curl -sL https://download.clearlinux.org/latest)
$ curl -LO "https://download.clearlinux.org/current/clear-${latest_version}-containers.img.xz"
$ unxz clear-${latest_version}-containers.img.xz
$ sudo mkdir -p /usr/share/clear-containers/
$ sudo install --owner root --group root --mode 0644 clear-${latest_version}-containers.img /usr/share/clear-containers/
$ sudo ln -fs /usr/share/clear-containers/clear-${latest_version}-containers.img /usr/share/clear-containers/clear-containers.img
```

#### Get virtc

_Download virtcontainers project_
```
$ go get github.com/kata-containers/runtime/virtcontainers
```

_Build and setup your environment_
```
$ cd $GOPATH/src/github.com/kata-containers/runtime/virtcontainers
$ go build -o virtc hack/virtc/main.go
$ sudo -E bash ./utils/virtcontainers-setup.sh
```

`virtcontainers-setup.sh` setup your environment performing different tasks. Particularly, it creates a __busybox__ bundle, and it creates CNI configuration files needed to run `virtc` with CNI plugins.

### Get cc-proxy (optional)

If you plan to start `virtc` with the hyperstart agent, you will have to use [cc-proxy](https://github.com/clearcontainers/proxy) as a proxy, meaning you have to perform extra steps to setup your environment.

```
$ go get github.com/clearcontainers/proxy
$ cd $GOPATH/src/github.com/clearcontainers/proxy
$ make
$ sudo make install
```
If you want to see the traces from the proxy when `virtc` will run, you can manually start it with appropriate debug level:

```
$ sudo /usr/libexec/clearcontainers/cc-proxy -v 3
```
This will generate output similar to the following:
```
I0410 08:58:49.058881    5384 proxy.go:521] listening on /var/run/clearcontainers/proxy.sock
I0410 08:58:49.059044    5384 proxy.go:566] proxy started
```
The proxy socket specified in the example log output has to be used as `virtc`'s `--proxy-url` option.

### Get cc-shim (optional)

If you plan to start `virtc` with the hyperstart agent (implying the use of `cc-proxy` as a proxy), you will have to rely on [cc-shim](https://github.com/clearcontainers/shim) in order to interact with the process running inside your container.
First, you will have to perform extra steps to setup your environment.

```
$ go get github.com/clearcontainers/shim
$ cd $GOPATH/src/github.com/clearcontainers/shim && ./autogen.sh
$ make
$ sudo make install
```

The shim will be installed at the following location: `/usr/libexec/clear-containers/cc-shim`. There will be three cases where you will be able to interact with your container's process through `cc-shim`:

_Start a new container_

```
# ./virtc container start --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
_Execute a new process on a running container_
```
# ./virtc container enter --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
_Start a sandbox with container(s) previously created_
```
# ./virtc sandbox start --id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
Notice that in both cases, the `--sandbox-id` and `--id` options have been defined when previously creating a sandbox and a container. 

### Run virtc

All following commands __MUST__ be run as root. By default, and unless you decide to modify it and rebuild it, `virtc` starts empty sandboxes (no container started).

#### Run a new sandbox (Create + Start)
```
# ./virtc sandbox run --agent="hyperstart" --network="CNI" --proxy="ccProxy" --proxy-url="unix:///var/run/clearcontainers/proxy.sock" --shim="ccShim" --shim-path="/usr/libexec/cc-shim"
```
#### Create a new sandbox
```
# ./virtc sandbox run --agent="hyperstart" --network="CNI" --proxy="ccProxy" --proxy-url="unix:///var/run/clearcontainers/proxy.sock" --shim="ccShim" --shim-path="/usr/libexec/cc-shim"
```
This will generate output similar to the following:
```
Sandbox 306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 created
```

#### Start an existing sandbox
```
# ./virtc sandbox start --id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Sandbox 306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 started
```

#### Stop an existing sandbox
```
# ./virtc sandbox stop --id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Sandbox 306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 stopped
```

#### Get the status of an existing sandbox and its containers
```
# ./virtc sandbox status --id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following (assuming the sandbox has been started):
```
SB ID                                  STATE   HYPERVISOR      AGENT
306ecdcf-0a6f-4a06-a03e-86a7b868ffc8    running qemu            hyperstart

CONTAINER ID    STATE
```

#### Delete an existing sandbox
```
# ./virtc sandbox delete --id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Sandbox 306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 deleted
```

#### List all existing sandboxes
```
# ./virtc sandbox list
```
This should generate that kind of output
```
SB ID                                  STATE   HYPERVISOR      AGENT
306ecdcf-0a6f-4a06-a03e-86a7b868ffc8    running qemu            hyperstart
92d73f74-4514-4a0d-81df-db1cc4c59100    running qemu            hyperstart
7088148c-049b-4be7-b1be-89b3ae3c551c    ready   qemu            hyperstart
6d57654e-4804-4a91-b72d-b5fe375ed3e1    ready   qemu            hyperstart
```

#### Create a new container
```
# ./virtc container create --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 --rootfs="/tmp/bundles/busybox/rootfs" --cmd="/bin/ifconfig" --console="/dev/pts/30"
```
This will generate output similar to the following:
```
Container 1 created
```
__Note:__ The option `--console` can be any existing console.
Don't try to provide `$(tty)` as it is your current console, and you would not be
able to get your console back as the shim would be listening to this indefinitely.
Instead, you would prefer to open a new shell and get the `$(tty)` from this shell.
That way, you make sure you have a dedicated input/output terminal. 

#### Start an existing container
```
# ./virtc container start --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Container 1 started
```

#### Run a new process on an existing container
```
# ./virtc container enter --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8 --cmd="/bin/ps" --console="/dev/pts/30"
```
This will generate output similar to the following:
```
Container 1 entered
```
__Note:__ The option `--console` can be any existing console.
Don't try to provide `$(tty)` as it is your current console, and you would not be
able to get your console back as the shim would be listening to this indefinitely.
Instead, you would prefer to open a new shell and get the `$(tty)` from this shell.
That way, you make sure you have a dedicated input/output terminal.

#### Stop an existing container
```
# ./virtc container stop --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Container 1 stopped
```

#### Delete an existing container
```
# ./virtc container delete --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following:
```
Container 1 deleted
```

#### Get the status of an existing container
```
# ./virtc container status --id=1 --sandbox-id=306ecdcf-0a6f-4a06-a03e-86a7b868ffc8
```
This will generate output similar to the following (assuming the container has been started):
```
CONTAINER ID    STATE
1               running
```
