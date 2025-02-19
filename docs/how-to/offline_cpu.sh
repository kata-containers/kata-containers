#!/usr/bin/env bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Offline SOS CPUs except BSP before launch UOS

[ $(id -u) -eq 0 ] || { echo >&2 "ERROR: run as root"; exit 1; }

for i in $(ls -d /sys/devices/system/cpu/cpu[1-9]*); do
        online=`cat $i/online`
        idx=`echo $i | tr -cd "[0-9]"`
        echo "INFO:$0: cpu$idx online=$online"
        if [ "$online" = "1" ]; then
                echo 0 > $i/online
                while [ "$online" = "1" ]; do
                        sleep 1
                        echo 0 > $i/online
                        online=`cat $i/online`
                done
        fi
done

