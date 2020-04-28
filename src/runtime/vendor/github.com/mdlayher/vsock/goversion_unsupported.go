//+build !go1.11

package vsock

func init() {
	// Intentionally break compilation on unsupported versions of Go, but
	// produce a somewhat informative build failure output.
	UpgradeGoCompilerToUseThisPackage
}
