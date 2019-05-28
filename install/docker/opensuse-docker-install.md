# Install Docker for Kata Containers on openSUSE

> **Note:**
>
> - This guide assumes you have
>   [already installed the Kata Containers packages](../opensuse-installation-guide.md).

1. Install the latest version of Docker with the following commands:

   > **Notes:**
   >
   > - This step is only required if Docker is not installed on the system.
   > - Docker version 18.09 [removed devicemapper support](https://github.com/kata-containers/documentation/issues/373).
   >   If you wish to use a block based backend, see the options listed on https://github.com/kata-containers/documentation/issues/407.

   ```bash
   $ sudo zypper -n install docker
   ```

   For more information on installing Docker please refer to the
   [Docker Guide](https://software.opensuse.org/package/docker).

2. Configure the Docker daemon to use Kata Containers by default, with one of the following methods:

   1. Specify the runtime options in `/etc/sysconfig/docker`:

       ```bash
       $ DOCKER_SYSCONFIG=/etc/sysconfig/docker
       # Add kata-runtime to the list of available runtimes, if not already listed
       $ grep -qE "^ *DOCKER_OPTS=.+--add-runtime[= ] *kata-runtime" $DOCKER_SYSCONFIG || sudo -E sed -i -E "s|^( *DOCKER_OPTS=.+)\" *$|\1 --add-runtime kata-runtime=/usr/bin/kata-runtime\"|g" $DOCKER_SYSCONFIG
       # If a current default runtime is specified, overwrite it with kata-runtime
       $ sudo -E sed -i -E "s|^( *DOCKER_OPTS=.+--default-runtime[= ] *)[^ \"]+(.*\"$)|\1kata-runtime\2|g" $DOCKER_SYSCONFIG
       # Add kata-runtime as default runtime, if no default runtime is specified
       $ grep -qE "^ *DOCKER_OPTS=.+--default-runtime" $DOCKER_SYSCONFIG || sudo -E sed -i -E "s|^( *DOCKER_OPTS=.+)(\"$)|\1 --default-runtime=kata-runtime\2|g" $DOCKER_SYSCONFIG
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
