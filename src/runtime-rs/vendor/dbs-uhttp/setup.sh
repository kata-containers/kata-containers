#!/bin/bash

docker run --rm -ti \
	--volume $(pwd):/dbs-uhttp \
	--workdir /dbs-uhttp \
	--security-opt seccomp=unconfined \
	rustvmm/dev:v15 /bin/bash

#	--privileged \
