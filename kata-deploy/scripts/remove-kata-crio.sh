#!/bin/sh
echo "deleting kata artifacts"
rm -rf /opt/kata/
mv /etc/crio/crio.conf.bak /etc/crio/crio.conf
