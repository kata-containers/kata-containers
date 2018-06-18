// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package virtcontainers

import (
	"fmt"

	cniTypes "github.com/containernetworking/cni/pkg/types"
	cniV2Types "github.com/containernetworking/cni/pkg/types/020"
	cniLatestTypes "github.com/containernetworking/cni/pkg/types/current"
	cniPlugin "github.com/kata-containers/runtime/virtcontainers/pkg/cni"
	"github.com/sirupsen/logrus"
)

// CniPrimaryInterface Name chosen for the primary interface
// If CNI ever support multiple primary interfaces this should be revisited
const CniPrimaryInterface = "eth0"

// cni is a network implementation for the CNI plugin.
type cni struct{}

// Logger returns a logrus logger appropriate for logging cni messages
func (n *cni) Logger() *logrus.Entry {
	return virtLog.WithField("subsystem", "cni")
}

func cniDNSToDNSInfo(cniDNS cniTypes.DNS) DNSInfo {
	return DNSInfo{
		Servers:  cniDNS.Nameservers,
		Domain:   cniDNS.Domain,
		Searches: cniDNS.Search,
		Options:  cniDNS.Options,
	}
}

func convertLatestCNIResult(result *cniLatestTypes.Result) NetworkInfo {
	return NetworkInfo{
		DNS: cniDNSToDNSInfo(result.DNS),
	}
}

func convertV2CNIResult(result *cniV2Types.Result) NetworkInfo {
	return NetworkInfo{
		DNS: cniDNSToDNSInfo(result.DNS),
	}
}

func convertCNIResult(cniResult cniTypes.Result) (NetworkInfo, error) {
	switch result := cniResult.(type) {
	case *cniLatestTypes.Result:
		return convertLatestCNIResult(result), nil
	case *cniV2Types.Result:
		return convertV2CNIResult(result), nil
	default:
		return NetworkInfo{}, fmt.Errorf("Unknown CNI result type %T", result)
	}
}

func (n *cni) invokePluginsAdd(sandbox *Sandbox, networkNS *NetworkNamespace) (*NetworkInfo, error) {
	netPlugin, err := cniPlugin.NewNetworkPlugin()
	if err != nil {
		return nil, err
	}

	// Note: In the case of multus or cni-genie this will return only the results
	// corresponding to the primary interface. The remaining results need to be
	// derived
	result, err := netPlugin.AddNetwork(sandbox.id, networkNS.NetNsPath, CniPrimaryInterface)
	if err != nil {
		return nil, err
	}

	netInfo, err := convertCNIResult(result)
	if err != nil {
		return nil, err
	}

	// We do not care about this for now but
	// If present, the CNI DNS result has to be updated in resolv.conf
	// if the kubelet has not supplied it already
	n.Logger().Infof("AddNetwork results %s", result.String())

	return &netInfo, nil
}

func (n *cni) invokePluginsDelete(sandbox *Sandbox, networkNS NetworkNamespace) error {
	netPlugin, err := cniPlugin.NewNetworkPlugin()
	if err != nil {
		return err
	}

	err = netPlugin.RemoveNetwork(sandbox.id, networkNS.NetNsPath, CniPrimaryInterface)
	if err != nil {
		return err
	}

	return nil
}

func (n *cni) updateEndpointsFromScan(networkNS *NetworkNamespace, netInfo *NetworkInfo, config NetworkConfig) error {
	endpoints, err := createEndpointsFromScan(networkNS.NetNsPath, config)
	if err != nil {
		return err
	}

	for _, endpoint := range endpoints {
		if CniPrimaryInterface == endpoint.Name() {
			prop := endpoint.Properties()
			prop.DNS = netInfo.DNS
			endpoint.SetProperties(prop)
			break
		}
	}

	networkNS.Endpoints = endpoints
	return nil
}

// init initializes the network, setting a new network namespace for the CNI network.
func (n *cni) init(config NetworkConfig) (string, bool, error) {
	return initNetworkCommon(config)
}

// run runs a callback in the specified network namespace.
func (n *cni) run(networkNSPath string, cb func() error) error {
	return runNetworkCommon(networkNSPath, cb)
}

// add adds all needed interfaces inside the network namespace for the CNI network.
func (n *cni) add(sandbox *Sandbox, config NetworkConfig, netNsPath string, netNsCreated bool) (NetworkNamespace, error) {

	networkNS := NetworkNamespace{
		NetNsPath:    netNsPath,
		NetNsCreated: netNsCreated,
	}

	netInfo, err := n.invokePluginsAdd(sandbox, &networkNS)
	if err != nil {
		return NetworkNamespace{}, err
	}

	if err := n.updateEndpointsFromScan(&networkNS, netInfo, config); err != nil {
		return NetworkNamespace{}, err
	}

	if err := addNetworkCommon(sandbox, &networkNS); err != nil {
		return NetworkNamespace{}, err
	}

	return networkNS, nil
}

// remove network endpoints in the network namespace. It also deletes the network
// namespace in case the namespace has been created by us.
func (n *cni) remove(sandbox *Sandbox, networkNS NetworkNamespace, netNsCreated bool) error {
	if err := removeNetworkCommon(networkNS, netNsCreated); err != nil {
		return err
	}

	if err := n.invokePluginsDelete(sandbox, networkNS); err != nil {
		return err
	}

	if netNsCreated {
		return deleteNetNS(networkNS.NetNsPath)
	}

	return nil
}
