# Using a debugger with the runtime

Setting up a debugger for the runtime is pretty complex: the shim is a server
process that is run by the runtime manager (containerd/CRI-O), and controlled by
sending gRPC requests to it.
Starting the shim with a debugger then just gives you a process that waits for
commands on its socket, and if the runtime manager doesn't start it, it won't
send request to it.

A first method is to attach a debugger to the process that was started by the
runtime manager.
If the issue you're trying to debug is not located at container creation, this
is probably the easiest method.

The other method involves a script that is placed in between the runtime manager
and the actual shim binary. This allows to start the shim with a debugger, and
wait for a client debugger connection before execution, allowing debugging of the
kata runtime from the very beginning.

## Prerequisite

At the time of writing, a debugger was used only with the go shim, but a similar
process should be doable with runtime-rs. This documentation will be enhanced
with rust-specific instructions later on.

In order to debug the go runtime, you need to use the [Delve debugger](https://github.com/go-delve/delve).

You will also need to build the shim binary with debug flags to make sure symbols
are available to the debugger.
Typically, the flags should be: `-gcflags=all=-N -l`

## Attach to the running process

To attach the debugger to the running process, all you need is to let the container
start as usual, then use the following command with `dlv`:

`$ dlv attach [pid of your kata shim]`

If you need to use your debugger remotely, you can use the following on your target
machine:

`$ dlv attach [pid of your kata shim] --headless --listen=[IP:port]`

then from your client computer:

`$ dlv connect [IP:port]`

## Make CRI-O/containerd start the shim with the debugger

You can use the [this script](../tools/containerd-shim-katadbg-v2) to make the
shim binary executed through a debugger, and make the debugger wait for a client
connection before running the shim.
This allows starting your container, connecting your debugger, and controlling the
shim execution from the beginning.

### Adapt the script to your setup

You need to edit the script itself to give it the actual binary
to execute.
Locate the following line in the script, and set the path accordingly.

```bash
SHIM_BINARY=
```

You may also need to edit the `PATH` variable set within the script,
to make sure that the `dlv` binary is accessible.

### Configure your runtime manager to use the script

Using either containerd or CRI-O, you will need to have a runtime class that
uses the script in place of the actual runtime binary.
To do that, we will create a separate runtime class dedicated to debugging.

- **For containerd**:
Make sure that the `containerd-shim-katadbg-v2` script is available to containerd
(putting it in the same folder as your regular kata shim typically).
Then edit the containerd configuration, and add the following runtime configuration,.

```toml
[plugins]
  [plugins."io.containerd.grpc.v1.cri"]
    [plugins."io.containerd.grpc.v1.cri".containerd]
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.katadbg]
          runtime_type = "io.containerd.katadbg.v2"
```

- **For CRI-O**:
Copy your existing kata runtime configuration from `/etc/crio/crio.conf.d/`, and
make a new one with the name `katadbg`, and the runtime_path set to the location
of the script.

E.g:

```toml
[crio.runtime.runtimes.katadbg]
  runtime_path = "/usr/local/bin/containerd-shim-katadbg-v2"
  runtime_root = "/run/vc"
  runtime_type = "vm"
  privileged_without_host_devices = true
  runtime_config_path = "/usr/share/defaults/kata-containers/configuration.toml"
 ```

NOTE: for CRI-O, the name of the runtime class doesn't need to match the name of the
script. But for consistency, we're using `katadbg` here too.

### Start your container and connect to the debugger

Once the above configuration is in place, you can start your container, using
your `katadbg` runtime class.

E.g: `$ crictl runp --runtime=katadbg sandbox.json`

The command will hang, and you can see that a `dlv` process is started

```
$ ps aux | grep dlv
root        9137  1.4  6.8 6231104 273980 pts/10 Sl   15:04   0:02 dlv exec /go/src/github.com/kata-containers/kata-containers/src/runtime/__debug_bin --headless --listen=:12345 --accept-multiclient -r stdout:/tmp/shim_output_oMC6Jo -r stderr:/tmp/shim_output_oMC6Jo -- -namespace default -address  -publish-binary /usr/local/bin/crio -id 0bc23d2208d4ff8c407a80cd5635610e772cae36c73d512824490ef671be9293 -debug start
```

Then you can use the `dlv` debugger to connect to it:

```
$ dlv connect localhost:12345
Type 'help' for list of commands.
(dlv)
```

Before doing anything else, you need to to enable `follow-exec` mode in delve.
This is because the first thing that the shim will do is to daemonize itself,
i.e: start itself as a subprocess, and exit. So you really want the debugger
to attach to the child process.

```
(dlv) target follow-exec -on .*/__debug_bin
```

Note that we are providing a regular expression to filter the name of the binary.
This is to make sure that the debugger attaches to the runtime shim, and not
to other subprocesses (hypervisor typically).

To ease this process, we recommand the use of an init file containing the above
command.

```
$ cat dlv.ini
target follow-exec -on .*/__debug_bin
$ dlv connect localhost:12345 --init=dlv.ini
Type 'help' for list of commands.
(dlv)
```

Once this is done, you can set breakpoints, and use the `continue` keyword to
start the execution of the shim.

You can also use a different client, like VSCode, to connect to it.
A typical `launch.json` configuration for VSCode would look like:

```yaml
[...]
{
    "name": "Connect to the debugger",
    "type": "go",
    "request": "attach",
    "mode": "remote",
    "port": 12345,
    "host": "127.0.0.1",
}
[...]
```

NOTE: VSCode's go extension doesn't seem to support the `follow-exec` mode from
Delve. So if you want to use VScode, you'll still need to use a commandline
`dlv` client to set the `follow-exec` flag.

## Caveats

Debugging takes time, and there are a lot of timeouts going on in a Kubernetes
environments. It is very possible that while you're debugging, some processes
will timeout and cancel the container execution, possibly breaking your debugging
session.

You can mitigate that by increasing the timeouts in the different components
involved in your environment.
