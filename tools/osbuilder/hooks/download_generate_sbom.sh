#!/bin/bash -eux

trivy fs /rootfs -f cyclonedx -o /kata-containers/sbom.cdx 2>&1 | tee /kata-containers/trivy.log

gzip /kata-containers/sbom.cdx
