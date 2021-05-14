# Test Kata using ccloudvm

* [How to use Kata workloads for `ccloudvm`](#how-to-use-kata-workloads-for-ccloudvm)
    * [Create Docker\* and Kata Containers virtualized environment](#create-docker-and-kata-containers-virtualized-environment)

***

The [ccloudvm](https://github.com/intel/ccloudvm/) tool is a command
to create development and demo environments. The tool sets up these development
environments inside a virtual machine.

## How to use Kata workloads for `ccloudvm`

- Follow the `ccloudvm` [install instructions](https://github.com/intel/ccloudvm/#introduction)

### Create Docker\* and Kata Containers virtualized environment

Create a virtual machine with Docker and Kata containers.

```bash
$ ccloudvm create --name YOUR_VM_NAME $PWD/kata-docker-xenial.yaml
$ ccloudvm connect YOUR_VM_NAME
```

You are ready to use Kata with docker in a virtualized environment.

See `ccloudvm` [documentation](https://github.com/intel/ccloudvm/#configurable-cloud-vm-ccloudvm) for advanced usage.
