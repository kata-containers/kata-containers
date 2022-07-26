package virtcontainers

import "os/exec"

func startInShimNS(cmd *exec.Cmd) error {
	// Create nydusd in shim netns as it needs to access host network
	return cmd.Start()
}
