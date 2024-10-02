#!/bin/bash
pkill hauler
sleep 3
mkdir -p /opt/hauler/.hauler
/usr/local/bin/hauler store load --store /opt/hauler/store /etc/super/opt/hauler/*.zst
nohup /usr/local/bin/hauler store serve fileserver --store /opt/hauler/store --directory /opt/hauler/registry &
/usr/local/bin/hauler store serve registry --store /opt/hauler/store --directory /opt/hauler/registry
