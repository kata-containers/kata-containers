# Install Kata Containers on Debian

1. Install the unsatisfied dependencies

  Kata Containers packages depends on a version of `librbd1` that's not yet available in the `stable` repo.
  A more recent version of `librbd1` can be installed from the `unstable` repo: https://packages.debian.org/sid/librbd1

  Add `unstable` repo to `/etc/apt/sources.list.d/unstable.list` sources list:
  
  ```bash
  $ sudo sh -c "echo '# for unstable packages
  deb http://ftp.debian.org/debian/ unstable main contrib non-free
  deb-src http://ftp.debian.org/debian/ unstable main contrib non-free' > /etc/apt/sources.list.d/unstable.list"
  ```
  
  Set the repository to a lower priority than stable, to ensures that APT will prefer stable packages over unstable ones. This can be specified in `/etc/apt/preferences.d/unstable`:
  
  ```bash
  $ sudo sh -c "echo 'Package: *
  Pin: release a=unstable
  Pin-Priority: 10' >> /etc/apt/preferences.d/unstable"
  ```

  Finally, install `librbd1`:

  ```bash 
  $ sudo apt-get update && sudo apt-get install -y -t unstable librbd1
  ```

2. Install the Kata Containers components with the following commands:

   ```bash
   $ export DEBIAN_FRONTEND=noninteractive
   $ ARCH=$(arch)
   $ source /etc/os-release
   $ sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/Debian_${VERSION_ID}/ /' > /etc/apt/sources.list.d/kata-containers.list"
   $ curl -sL  http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/Debian_${VERSION_ID}/Release.key | sudo apt-key add -
   $ sudo -E apt-get update
   $ sudo -E apt-get -y install kata-runtime kata-proxy kata-shim
   ```

3. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/ubuntu-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)
