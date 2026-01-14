# Table of Contents

**Note:**: This guide used to contain an end-to-end flow to build a
custom Kata containers root filesystem with QAT out-of-tree SR-IOV virtual
function driver and run QAT enabled containers. The former is no longer necessary
so the instructions are dropped. If the use-case is still of interest, please file
an issue in either of the QAT Kubernetes specific repos linked below.

# Introduction

Intel® QuickAssist Technology (QAT) provides hardware acceleration
for security (cryptography) and compression. Kata Containers can enable
these acceleration functions for containers using QAT SR-IOV with the
support from [Intel QAT Device Plugin for Kubernetes](https://github.com/intel/intel-device-plugins-for-kubernetes)
or [Intel QAT DRA Resource Driver for Kubernetes](https://github.com/intel/intel-resource-drivers-for-kubernetes).

## More Information

[Intel® QuickAssist Technology at `01.org`](https://www.intel.com/content/www/us/en/developer/topic-technology/open/quick-assist-technology/overview.html)

[Intel® QuickAssist Technology Engine for OpenSSL](https://github.com/intel/QAT_Engine)

[Intel Device Plugin for Kubernetes](https://github.com/intel/intel-device-plugins-for-kubernetes)

[Intel DRA Resource Driver for Kubernetes](https://github.com/intel/intel-resource-drivers-for-kubernetes)

[Intel® QuickAssist Technology for Crypto Poll Mode Driver](https://dpdk-docs.readthedocs.io/en/latest/cryptodevs/qat.html)
