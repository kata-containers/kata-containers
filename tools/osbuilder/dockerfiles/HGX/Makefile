.PHONY docker_build
docker_build:
	docker build --label hgx --tag hgx:latest .

.PHONY all
all: docker_build
	docker run -it -v /dev:/dev -v $(PWD)/output:/output hgx

