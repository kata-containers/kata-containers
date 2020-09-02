# Install Docker for Kata Containers on RHEL

> **Note:**
>
> - This guide assumes you have
>   [already installed the Kata Containers packages](../rhel-installation-guide.md).

1. Install the latest version of Docker with the following commands:

   > **Notes:**
   >
   > - This step is only required if Docker is not installed on the system.
   > - Docker version 18.09 [removed devicemapper support](https://github.com/kata-containers/documentation/issues/373).
   >   If you wish to use a block based backend, see the options listed on https://github.com/kata-containers/documentation/issues/407.

   ```bash
   $ export rhel_devtoolset_version="7"
   $ sudo subscription-manager repos --enable=rhel-${rhel_devtoolset_version}-server-extras-rpms
   $ sudo yum -y install docker && systemctl enable --now docker
   ```

   For more information on installing Docker please refer to the
   [Docker Guide](https://access.redhat.com/documentation/en-us/red_hat_enterprise_linux_atomic_host/7/html-single/getting_started_with_containers/#getting_docker_in_rhel_7).

2. Configure Docker to use Kata Containers by default with **ONE** of the following methods:

   1. systemd (this is the default and is applied automatically if you select the
      [automatic installation](../../install/README.md#automatic-installation) option)

       ```bash
       $ sudo mkdir -p /etc/systemd/system/docker.service.d/
       $ cat <<EOF | sudo tee /etc/systemd/system/docker.service.d/kata-containers.conf
       [Service]
       ExecStart=
       ExecStart=/usr/bin/dockerd -D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime
       EOF
       ```

   2. Docker `daemon.json`

      Create docker configuration folder.

      ```
      $ sudo mkdir -p /etc/docker
      ```

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
