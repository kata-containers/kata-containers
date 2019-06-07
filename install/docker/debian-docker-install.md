# Install Docker for Kata Containers on Debian

> **Note:**
>
> - This guide assumes you have
>   [already installed the Kata Containers packages](../debian-installation-guide.md).
> - This guide allows for installation with `systemd` or `sysVinit` init systems.

1. Install the latest version of Docker with the following commands:

   > **Notes:**
   >
   > - This step is only required if Docker is not installed on the system.
   > - Docker version 18.09 [removed devicemapper support](https://github.com/kata-containers/documentation/issues/373).
   >   If you wish to use a block based backend, see the options listed on https://github.com/kata-containers/documentation/issues/407.

   ```bash
   $ sudo apt-get -y install apt-transport-https ca-certificates curl gnupg2 software-properties-common  
   $ curl -fsSL https://download.docker.com/linux/$(. /etc/os-release; echo "$ID")/gpg | sudo apt-key add -
   $ sudo add-apt-repository "deb https://download.docker.com/linux/$(. /etc/os-release; echo "$ID") $(lsb_release -cs) stable"
   $ sudo apt-get update
   $ sudo -E apt-get -y install docker-ce
   ```

   For more information on installing Docker please refer to the
   [Docker Guide](https://docs.docker.com/engine/installation/linux/debian).

2. Configure Docker to use Kata Containers by default with ONE of the following methods:

a. `sysVinit`
    
    - with `sysVinit`, docker config is stored in `/etc/default/docker`, edit the options similar to the following:
       
    ```sh
    $ sudo sh -c "echo '# specify docker runtime for kata-containers
    DOCKER_OPTS=\"-D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime\"' >> /etc/default/docker"
    ```
    
b. systemd

    ```bash
    $ sudo mkdir -p /etc/systemd/system/docker.service.d/
    $ cat <<EOF | sudo tee /etc/systemd/system/docker.service.d/kata-containers.conf
    [Service]
    ExecStart=
    ExecStart=/usr/bin/dockerd -D --add-runtime kata-runtime=/usr/bin/kata-runtime --default-runtime=kata-runtime
    EOF
    ```

c. Docker `daemon.json`

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

3. Restart the Docker systemd service with one of the following (depending on init choice):

    a. `sysVinit`
  
    ```sh
    $ sudo /etc/init.d/docker stop
    $ sudo /etc/init.d/docker start
    ```

    To watch for errors:

    ```sh
    $ tail -f /var/log/docker.log
    ```
    
    b. systemd

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
