#!/bin/bash

# if NOT DEBUG, then close VM via firewall and stopping from SSH, and TTY terminal
if ! grep -q 'sp-debug=true' /proc/cmdline; then
    # Stop services
    systemctl stop serial-getty@ttyS0.service
    systemctl stop ssh

    # Disable services, but this is not working on RO system
    # systemctl disable serial-getty@ttyS0.service
    # systemctl disable ssh

    # Set default policy to DROP for INPUT
    iptables -P INPUT DROP

    # Allow established and related connections
    iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT

    # Allow all traffic on the loopback interface
    iptables -A INPUT -i lo -j ACCEPT
    iptables -A OUTPUT -o lo -j ACCEPT

    # Allow DNS requests
    iptables -A INPUT -p udp --dport 53 -j ACCEPT
    iptables -A INPUT -p udp --sport 53 -j ACCEPT

    # Allow API server (TCP 443 for HTTPS)
    iptables -A INPUT -p tcp --dport 443 -s 10.43.0.1 -j ACCEPT

    # Allow incoming traffic in the cluster network
    # @TODO this will ignore NetworkPolicies in k8s, refactor in future
    iptables -I INPUT -s 10.43.0.0/16 -j ACCEPT
    iptables -I INPUT -s 10.42.0.0/16 -j ACCEPT
fi

if grep -q 'sp-debug=true' /proc/cmdline; then
  chmod 400 /sp/authorized_keys || echo 'authorized_keys not found'
fi