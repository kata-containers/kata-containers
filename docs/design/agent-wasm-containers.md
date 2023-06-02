# Wasm Containers Support for Agent

**WebAssembly (Wasm)** is a binary instruction format designed for a stack-based virtual machine. As a new feature, wasm containers are **not** enabled by default in `kata-agent`. It will increase the memory footprint of the `kata-agent` by about `44.7%` (from `18610448B` to `26935136B` with `x86_64-unknown-linux-gnu`), so please decide **carefully** whether to enable this feature. We currently choose [wasmtime](https://github.com/bytecodealliance/wasmtime) as the **built-in** wasm runtime to execute wasm files. In order to run wasm containers inside `kata-agent`, please make sure `kata-agent` in the image file compiled with the feature `wasm-runtime` and your container created with the **annotation** `"io.katacontainers.platform.wasi/wasm32": "yes"` or `"true"`.

> **Note:**
> 
> - Only the following **architectures** support wasm containers
> 	- x86_64
> 	- x86
> 	- aarch64
> 	- arm
> 	- riscv64
> 	- riscv32
>

## Usage

The following is a sample workflow of running a wasm container in the `kata-agent`.

1. Enable the the `wasm-runtime` feature to `kata-agent`.

	```shell
	$ sed -i -e 's/WASM_RUNTIME := no$/WASM_RUNTIME := yes$/g' src/agent/Makefile
	```

2. Compile and replace `kata-agent` in the `image` according to the documentation [Developer Guide](../Developer-Guide.md#build-a-custom-kata-agent---optional).

3. Prepare a directory with wasm files in it. (Take `/tmp/hello.wasm` as an example).

4. Create a pod and start a container with the **annotation** mentioned above.
   
   	```shell
   	$ sh -c 'cat > pod_cfg.json <<-EOF
	{
		"metadata": {
			"name": "kata-wasm-pod"
		}
	}
	EOF'

	$ sh -c 'cat > ctr_cfg.json <<-EOF
	{
		"metadata": {
			"name": "kata-wasm-ctr"
		},
		"image": {
			"image": "docker.io/library/busybox:latest"
		},
		"mounts": [
			{
				"container_path": "/share",
				"host_path": "/tmp"
			}
		],
		"command": [
			"/share/hello.wasm"
		],
		"annotations": {
			"io.katacontainers.platform.wasi/wasm32": "true"
		}
	}
	EOF'

	$ pid=`sudo crictl runp -r kata pod_cfg.json`
	$ cid=`sudo crictl create $pid ctr_cfg.json pod_cfg.json`
	$ sudo crictl start $cid
   	```

## References

- [WebAssembly](https://webassembly.org)
- [Wasmtime](https://wasmtime.dev)
