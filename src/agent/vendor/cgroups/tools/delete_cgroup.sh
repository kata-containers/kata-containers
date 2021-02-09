#!/bin/sh
#
# Copyright (c) 2018 Levente Kurusa
#
# SPDX-License-Identifier: Apache-2.0 or MIT
#

CONTROL_GROUPS=`cargo test -- --list 2>/dev/null | egrep 'test$' | egrep -v '^src' | cut -d':' -f1`

echo This script will delete the control groups created by the create_cgroup.sh shell script.
echo
echo It may spit out some errors, but that is fine.
echo
echo For this, we will need your sudo privileges. Please do not trust this shell script and have a look to check that it does something that you are okay with.
sudo -v

for i in ${CONTROL_GROUPS}
	do sudo rmdir /sys/fs/cgroup/{blkio,cpu,cpuacct,cpuset,devices,freezer,hugetlb,memory,net_cls,net_prio,perf_event,pids}/$i/
done
