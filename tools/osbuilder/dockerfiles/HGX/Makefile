.PHONY: docker_build
docker_build:
	docker build --label hgx --tag hgx:latest .

.PHONY: gpu_kernel
gpu_kernel:
	cd ../../../packaging/kata-deploy/local-build/ && make kernel-nvidia-gpu-tarball
	mkdir -p ./kernel/
	cp -r ../../../packaging/kata-deploy/local-build/build/ ./kernel/

.PHONY: all
all: gpu_kernel docker_build
	docker run -it -v /dev:/dev -v $(PWD)/output:/output hgx -v ./kernel/:/kata-containers/tools/packaging/kata-deploy/local-build/build/

