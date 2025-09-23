.PHONY: docker_build
docker_build:
	docker build --label hgx --tag hgx:latest .

.PHONY: gpu_kernel
gpu_kernel:
	@if [ ! -d ./kernel/ ] || [ -z "$$(ls -A ./kernel/)" ]; then \
		cd ../../../packaging/kata-deploy/local-build/ && make kernel-nvidia-gpu-tarball; \
		mkdir -p ./kernel/; \
		cp -r ../../../packaging/kata-deploy/local-build/build/ ./kernel/; \
	else \
		echo "Skipping gpu_kernel: ./kernel/ already populated."; \
	fi

.PHONY: all
all: gpu_kernel docker_build
	docker run --privileged --rm -it -v /dev:/dev -v $(PWD)/output:/output hgx
