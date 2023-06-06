module github.com/kata-containers/tests/metrics/k8s

go 1.15

replace github.com/kata-containers/tests/metrics/exec => ../exec

require (
	github.com/kata-containers/tests/metrics/exec v0.0.0-00010101000000-000000000000
	github.com/pkg/errors v0.9.1
)
