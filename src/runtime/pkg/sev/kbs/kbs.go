// Copyright contributors to AMD SEV/-ES in Go
//
// SPDX-License-Identifier: Apache-2.0
//

// Package kbs can be used interact with simple-kbs, the key broker
// server for SEV and SEV-ES pre-attestation

package kbs

const (
	Offline           = "offline"
	OfflineSecretType = "bundle"
	OfflineSecretGuid = "e6f5a162-d67f-4750-a67c-5d065f2a9910"
	Online            = "online"
	OnlineBootParam   = "online_sev_kbc"
	OnlineSecretType  = "connection"
	OnlineSecretGuid  = "1ee27366-0c87-43a6-af48-28543eaf7cb0"
)

type GuestPreAttestationConfig struct {
	Proxy            string
	Keyset           string
	LaunchId         string
	KernelPath       string
	InitrdPath       string
	FwPath           string
	KernelParameters string
	CertChainPath    string
	SecretType       string
	SecretGuid       string
	Policy           uint32
}
