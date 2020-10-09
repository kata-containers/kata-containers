# Working with `crictl`

* [What's `cri-tools`](#whats-cri-tools)
* [Use `crictl` run Pods in Kata containers](#use-crictl-run-pods-in-kata-containers)
  * [Run `busybox` Pod](#run-busybox-pod)
    * [Run pod sandbox with config file](#run-pod-sandbox-with-config-file)
    * [Create container in the pod sandbox with config file](#create-container-in-the-pod-sandbox-with-config-file)
    * [Start container](#start-container)
  * [Run `redis` Pod](#run-redis-pod)
    * [Create `redis-server` Pod](#create-redis-server-pod)
    * [Create `redis-client` Pod](#create-redis-client-pod)
    * [Check `redis` server is working](#check-redis-server-is-working)

## What's `cri-tools`

[`cri-tools`](https://github.com/kubernetes-sigs/cri-tools) provides debugging and validation tools for Kubelet Container Runtime Interface (CRI).

`cri-tools` includes two tools: `crictl` and `critest`. `crictl` is the CLI for Kubelet CRI, in this document, we will show how to use `crictl` to run Pods in Kata containers.

> **Note:** `cri-tools` is only used for debugging and validation purpose, and don't use it to run production workloads.

> **Note:** For how to install and configure `cri-tools` with CRI runtimes like `containerd` or CRI-O, please also refer to other [howtos](./README.md).

## Use `crictl` run Pods in Kata containers

Sample config files in this document can be found [here](./data/crictl/).

### Run `busybox` Pod

#### Run pod sandbox with config file

```bash
$ sudo crictl runp -r kata sandbox_config.json
16a62b035940f9c7d79fd53e93902d15ad21f7f9b3735f1ac9f51d16539b836b

$ sudo crictl pods
POD ID              CREATED             STATE               NAME                NAMESPACE           ATTEMPT
16a62b035940f       21 seconds ago      Ready               busybox-pod                             0
```

#### Create container in the pod sandbox with config file

```bash
$ sudo crictl create 16a62b035940f container_config.json sandbox_config.json 
e6ca0e0f7f532686236b8b1f549e4878e4fe32ea6b599a5d684faf168b429202
```

List containers and check the container is in `Created` state:

```bash
$ sudo crictl ps -a
CONTAINER           IMAGE                              CREATED             STATE               NAME                ATTEMPT             POD ID
e6ca0e0f7f532       docker.io/library/busybox:latest   19 seconds ago      Created             busybox-container   0                   16a62b035940f
```

#### Start container

```bash
$ sudo crictl start e6ca0e0f7f532
e6ca0e0f7f532
```

List containers and we can see that the container state has changed from `Created` to `Running`:

```bash
$ sudo crictl ps
CONTAINER           IMAGE                              CREATED              STATE               NAME                ATTEMPT             POD ID
e6ca0e0f7f532       docker.io/library/busybox:latest   About a minute ago   Running             busybox-container   0                   16a62b035940f
```

And last we can `exec` into `busybox` container:

```bash
$ sudo crictl exec -it e6ca0e0f7f532 sh
```

And run commands in it:

```
/ # hostname 
busybox_host
/ # id
uid=0(root) gid=0(root)
```

### Run `redis` Pod

In this example, we will create two Pods: one is for `redis` server, and another one is `redis` client.

#### Create `redis-server` Pod

It's also possible to start a container within a single command:

```bash
$ sudo crictl run -r kata redis_server_container_config.json redis_server_sandbox_config.json
bb36e05c599125842c5193909c4de186b1cee3818f5d17b951b6a0422681ce4b
```

#### Create `redis-client` Pod

```bash
$ sudo crictl run -r kata redis_client_container_config.json redis_client_sandbox_config.json
e344346c5414e3f51f97f20b2262e0b7afe457750e94dc0edb109b94622fc693
```

After the new container started, we can check the running Pods and containers.

```bash
$ sudo crictl pods
POD ID              CREATED              STATE               NAME                NAMESPACE           ATTEMPT
469d08a7950e3       30 seconds ago       Ready               redis-client-pod                        0
02c12fdb08219       About a minute ago   Ready               redis-server-pod                        0

$ sudo crictl ps
CONTAINER           IMAGE                                  CREATED              STATE               NAME                ATTEMPT             POD ID
e344346c5414e       docker.io/library/redis:6.0.8-alpine   35 seconds ago       Running             redis-client        0                   469d08a7950e3
bb36e05c59912       docker.io/library/redis:6.0.8-alpine   About a minute ago   Running             redis-server        0                   02c12fdb08219
```

#### Check `redis` server is working

To connect to the `redis-server`. First we need to get the `redis-server`'s IP address.

```bash

$ server=$(sudo crictl inspectp 02c12fdb08219 | jq .status.network.ip | tr -d '"' )
$ echo $server
172.19.0.118
```

Launch `redis-cli` in the new Pod and connect server running at `172.19.0.118`.

```bash
$ sudo crictl exec -it e344346c5414e redis-cli -h $server
172.19.0.118:6379> get test-key
(nil)
172.19.0.118:6379> set test-key test-value
OK
172.19.0.118:6379> get test-key
"test-value"
```

Then back to `redis-server`, check if the `test-key` is set in server.

```bash
$ sudo crictl exec -it bb36e05c59912 redis-cli get test-key
"test-val"
```

Returned `test-val` is just set by `redis-cli` in `redis-client` Pod.
