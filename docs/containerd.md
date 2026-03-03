# containerd

Kata requires a [CRI]-compatible container runtimes. containerd is commonly used for Kata. We recommend installing containerd using your platform's package distribution mechanism. We recommend the latest version of containerd v2.[^1]


## Debian/Ubuntu

```sh
$ apt update
$ apt install containerd
$ systemctl status containerd
● containerd.service - containerd container runtime
     Loaded: loaded (/etc/systemd/system/containerd.service; enabled; preset: enabled)
    Drop-In: /etc/systemd/system/containerd.service.d
             └─http-proxy.conf
     Active: active (running) since Wed 2026-02-25 22:58:13 UTC; 5 days ago
       Docs: https://containerd.io
   Main PID: 3767885 (containerd)
      Tasks: 540
     Memory: 70.7G (peak: 70.8G)
        CPU: 4h 9min 26.153s
     CGroup: /runtime.slice/containerd.service
             ├─  12694 /usr/local/bin/container
```

## Pre-Built Releases

Many Linux distributions will not package the latest versions of containerd. If you find that your distribution provides very old versions of containerd, it's recommended to upgrade with the [pre-built releases](https://github.com/containerd/containerd/releases).

### Executable

Download the latest release of containerd:

```sh
$ wget https://github.com/containerd/containerd/releases/download/v${VERSION}/containerd-${VERSION}-linux-${PLATFORM}.tar.gz

# Extract to the current directory 
$ tar -xf ./containerd*.tar.gz

# Extract to root if you want it installed to its final location.
$ tar -C / -xf ./*.tar.gz
```

### Containerd Config

Containerd requires a config file at `/etc/containerd/config.toml`. This needs to be populated with a simple default config:

```sh
$ /usr/local/bin/containerd config default > /etc/containerd/config.toml
```

### runc

The default `runc` runtime needs to be installed for non-kata containers. More details can be found at the [containerd docs](https://github.com/containerd/containerd/blob/979c80d8a5d7fc7be34102a1ada53ae5a0ff09e8/docs/RUNC.md).

### Systemd Unit File

Install the systemd unit file:

```sh
$ wget -O /etc/systemd/system/containerd.service https://raw.githubusercontent.com/containerd/containerd/main/containerd.service
```

!!! info

    - You must modify the `ExecStart` line to the location of the installed containerd executable. 
    - containerd's `PATH` variable must allow it to find `containerd-shim-kata-v2`. You can do this by either creating a symlink from `/usr/local/bin/containerd-shim-kata-v2` to `/opt/kata/bin/containerd-shim-kata-v2` or by modifying containerd's `PATH` variable to search in `/opt/kata/bin/`. See the Environment= command in systemd.exec(5) for further details.


Reload systemd and start containerd:

```sh
$ systemctl daemon-reload
$ systemctl enable --now containerd
$ systemctl start containerd
```

More details can be found on the [containerd installation docs](https://github.com/containerd/containerd/blob/main/docs/getting-started.md).

## Enable CRI

If you're using Kubernetes, you must enable the containerd Container Runtime Interface (CRI) plugin:

```sh
$ ctr plugins ls | grep cri
io.containerd.cri.v1                      images                   -              ok        
io.containerd.cri.v1                      runtime                  linux/amd64    ok        
io.containerd.grpc.v1                     cri                      -              ok    
```

If these are not enabled, you'll need to remove it from the `disabled_plugins` section of the containerd config.


[^1]: Kata makes use of containerd's drop-in config merging in `/etc/containerd/config.d/` which is only available starting from containerd v2. containerd v1 may work, but some Kata features will not work as expected.