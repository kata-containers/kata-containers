# Installing Kata Containers on CentOS

> **Notes:**
>
> - Kata Containers packages are available for CentOS version 7 (currently `x86_64` only).
>
> - If you are installing on a system that already has Clear Containers or `runv` installed,
>   first read [the upgrading document](../Upgrading.md).
>
> - This install guide is specific to integration with Docker.  If you want to use Kata
>   Containers through Kubernetes, see the "Developer Guide" [here](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#if-you-want-to-run-kata-containers-with-kubernetes).

This step is only required in case Docker is not installed on the system.
1. Install the latest version of Docker with the following commands:

```bash
$ sudo yum -y install yum-utils
$ sudo yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
$ sudo yum -y install docker-ce
```
For more information on installing Docker please refer to the
[Docker Guide](https://docs.docker.com/engine/installation/linux/centos)

2. Install the Kata Containers components with the following commands:

> **Note:** The repository redirects the download content to use `http`, be aware that this installation channel is not secure.

```bash
$ source /etc/os-release
$ sudo -E VERSION_ID=$VERSION_ID yum-config-manager --add-repo \
"http://download.opensuse.org/repositories/home:/katacontainers:/release/CentOS_${VERSION_ID}/home:katacontainers:release.repo"
$ sudo -E yum -y install kata-runtime kata-proxy kata-shim
```

3. Configure Docker to use Kata Containers by default with the following commands:

```bash
$ sudo mkdir -p /etc/systemd/system/docker.service.d/
$ cat <<EOF | sudo tee /etc/systemd/system/docker.service.d/kata-containers.conf
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd -D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime
EOF
```

4. Restart the Docker systemd service with the following commands:

```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart docker
```

5. Run Kata Containers

You are now ready to run Kata Containers:

```
$ sudo docker run -ti busybox sh
```
