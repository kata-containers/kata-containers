# Installing Kata Containers on Ubuntu

> **Note:** Kata Containers is available for Ubuntu\* **16.04** and **17.10**.

Currently Kata Containers is currently only build for x86_64.


This step is only required in case Docker is not installed on the system.
1. Install the latest version of Docker with the following commands:

```bash
$ sudo -E apt-get -y install apt-transport-https ca-certificates wget software-properties-common
$ curl -sL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
$ arch=$(dpkg --print-architecture)
$ sudo -E add-apt-repository "deb [arch=${arch}] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable"
$ sudo -E apt-get update
$ sudo -E apt-get -y install docker-ce
```

For more information on installing Docker please refer to the
[Docker Guide](https://docs.docker.com/engine/installation/linux/ubuntu)

2. Install the Kata Containers components with the following commands:

> **Note:** The repository is downloading content using `http`, be aware that this installation channel is not secure.

```bash
$ sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/katacontainers:/release/xUbuntu_$(lsb_release -rs)/ /' > /etc/apt/sources.list.d/kata-containers.list"
$ curl -sL  http://download.opensuse.org/repositories/home:/katacontainers:/release/xUbuntu_$(lsb_release -rs)/Release.key | sudo apt-key add -
$ sudo -E apt-get update
$ sudo -E apt-get -y install kata-runtime kata-proxy kata-shim
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
