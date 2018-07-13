# Install Docker for Kata Containers on RHEL

> **Note:**
>
> - This guide assumes you have
>   [already installed the Kata Containers packages](../rhel-installation-guide.md).
>
> - If you do not want to copy or type all these instructions by hand, you can use the
>   [`kata-manager`](https://github.com/kata-containers/tests/blob/master/cmd/kata-manager/kata-manager.sh)
>   script to install the packaged system including your chosen container
>   manager. Alternatively, you can generate a runnable shell script from
>   individual documents using the
>   [`kata-doc-to-script`](https://github.com/kata-containers/tests/blob/master/.ci/kata-doc-to-script.sh) script.

1. Install the latest version of Docker with the following commands:

   > **Note:** This step is only required if Docker is not installed on the system.

   ```bash
   $ export rhel_devtoolset_version="7"
   $ sudo subscription-manager repos --enable=rhel-${rhel_devtoolset_version}-server-extras-rpms
   $ sudo yum -y install docker && systemctl enable --now docker
   ```

   For more information on installing Docker please refer to the
   [Docker Guide](https://access.redhat.com/documentation/en-us/red_hat_enterprise_linux_atomic_host/7/html-single/getting_started_with_containers/#getting_docker_in_rhel_7).

2. Configure Docker to use Kata Containers by default with one of the following methods:

    1. systemd

        ```bash
        $ sudo mkdir -p /etc/systemd/system/docker.service.d/
        $ cat <<EOF | sudo tee /etc/systemd/system/docker.service.d/kata-containers.conf
        [Service]
        ExecStart=
        ExecStart=/usr/bin/dockerd -D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime
        EOF
        ```

    2. Docker `daemon.json`

        Add the following definitions to `/etc/docker/daemon.json`:

        ```json
        {
          "default-runtime": "kata-runtime",
          "runtimes": {
            "kata-runtime": {
              "path": "/usr/bin/kata-runtime"
            }
          }
        }
        ```

3. Restart the Docker systemd service with the following commands:

   ```bash
   $ sudo systemctl daemon-reload
   $ sudo systemctl restart docker
   ```

4. Run Kata Containers

   You are now ready to run Kata Containers:

   ```bash
   $ sudo docker run busybox uname -a
   ```

   The previous command shows details of the kernel version running inside the
   container, which is different to the host kernel version.
