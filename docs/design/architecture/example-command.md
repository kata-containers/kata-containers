# Example command

The following containerd command creates a container. It is referred
to throughout the architecture document to help explain various points:

```bash
$ sudo ctr run --runtime "io.containerd.kata.v2" --rm -t "quay.io/libpod/ubuntu:latest" foo sh
```

This command requests that containerd:

- Create a container (`ctr run`).
- Use the Kata [shimv2](README.md#shim-v2-architecture) runtime (`--runtime "io.containerd.kata.v2"`).
- Delete the container when it [exits](README.md#workload-exit) (`--rm`).
- Attach the container to the user's terminal (`-t`).
- Use the Ubuntu Linux [container image](background.md#container-image)
  to create the container [rootfs](background.md#root-filesystem) that will become
  the [container environment](README.md#environments)
  (`quay.io/libpod/ubuntu:latest`).
- Create the container with the name "`foo`".
- Run the `sh(1)` command in the Ubuntu rootfs based container
  environment.

  The command specified here is referred to as the [workload](README.md#workload).

> **Note:**
>
> For the purposes of this document and to keep explanations
> simpler, we assume the user is running this command in the
> [host environment](README.md#environments).
