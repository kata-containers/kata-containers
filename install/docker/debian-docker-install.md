# Install Docker for Kata Containers on Debian

> **Note:**
>
> - This guide assumes you have
>   [already installed the Kata Containers packages](../debian-installation-guide.md).
> - this guide allows for installation with `systemd` or `sysVinit` init systems

1. Install Docker with the following commands:

   > **Notes:**
   >
   > - This step is only required if Docker is not installed on the system.
   > - Newer versions of Docker have
   >   [removed devicemapper support](https://github.com/kata-containers/documentation/issues/373)
   >   so the following commands install the latest version, which includes
   >   devicemapper support.
   > - To remove the lock on the docker package to allow it to be updated:
   >   ```sh
   >   $ sudo apt-mark unhold docker-ce
   >   ```

   ```bash
   $ sudo apt-get -y install apt-transport-https ca-certificates curl gnupg2 software-properties-common  
   $ curl -fsSL https://download.docker.com/linux/$(. /etc/os-release; echo "$ID")/gpg | sudo apt-key add -
   $ sudo add-apt-repository "deb https://download.docker.com/linux/$(. /etc/os-release; echo "$ID") $(lsb_release -cs) stable"
   $ sudo apt-get update
   $ sudo -E apt-get -y install --allow-downgrades docker-ce='18.06.2~ce~3-0~debian'
   $ sudo apt-mark hold docker-ce
   ```

   For more information on installing Docker please refer to the
   [Docker Guide](https://docs.docker.com/engine/installation/linux/debian).

2. Configure Docker to use Kata Containers by default with ONE of the following methods:

a. sysVinit
    
    - with sysVinit,  docker config is stored in `/etc/default/docker`, edit the options similar to the following: 
       
    ```
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

c. systemd Docker `daemon.json`

    Add the following definitions to `/etc/docker/daemon.json`:

    ```bash
    $ sudo sh -c "echo '{
      \"default-runtime\": \"kata-runtime\",
      \"runtimes\": {
        \"kata-runtime\": {
          \"path\": \"/usr/bin/kata-runtime\"
        }
      }
    }' >> /etc/docker/daemon.json"
    ```

3. Restart the Docker systemd service with one of the following (depending on init choice):

    a. sysVinit
  
    ```bash
    $ sudo /etc/init.d/docker stop
    $ sudo /etc/init.d/docker start
    ```

    to watch for errors:

    ```bash
    tail -f /var/log/docker.log
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

   
   
