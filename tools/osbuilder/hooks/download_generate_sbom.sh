#!/bin/bash -eux

trivy fs /rootfs -f cyclonedx -o /kata-containers/sbom.json 2>&1 | tee /kata-containers/trivy.log
