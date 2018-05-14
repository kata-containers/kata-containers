# Installing Kata Containers on Fedora

Note:
Kata Containers is available for Fedora\* versions **26** and **27**.

This step is only required in case Docker is not installed on the system.
1. Install the latest version of Docker with the following commands:

```
$ sudo dnf -y install dnf-plugins-core
$ sudo dnf config-manager --add-repo https://download.docker.com/linux/fedora/docker-ce.repo
$ sudo dnf makecache
$ sudo dnf -y install docker-ce
```

For more information on installing Docker please refer to the
[Docker Guide](https://docs.docker.com/engine/installation/linux/fedora)

2. Install the Kata Containers components with the following commands:

```
$ source /etc/os-release
$ sudo -E VERSION_ID=$VERSION_ID dnf config-manager --add-repo \
https://download.opensuse.org/repositories/home:/katacontainers:/release/Fedora\_$VERSION_ID/home:katacontainers:release.repo
$ sudo -E dnf -y install kata-runtime kata-proxy kata-shim
```

3. Configure Docker to use Kata Containers by default with the following commands:

```
$ sudo mkdir -p /etc/systemd/system/docker.service.d/
$ cat <<EOF | sudo tee /etc/systemd/system/docker.service.d/kata-containers.conf
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd -D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime
EOF
```

4. Restart the Docker systemd service with the following commands:

```
$ sudo systemctl daemon-reload
$ sudo systemctl restart docker
```

5. Run Kata Containers

You are now ready to run Kata Containers:

```
$ sudo docker run -ti busybox sh
```
