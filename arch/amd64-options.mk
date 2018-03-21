# Copyright (c) 2018 Intel Corporation
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Intel x86-64 settings

MACHINETYPE := pc
KERNELPARAMS :=
MACHINEACCELERATORS :=

# The CentOS/RHEL hypervisor binary is not called qemu-lite
ifeq (,$(filter-out centos rhel,$(distro)))
    QEMUCMD := qemu-system-x86_64
else
    QEMUCMD := qemu-lite-system-x86_64
endif

