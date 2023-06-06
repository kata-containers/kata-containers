module github.com/tests/metrics/storage/fio-k8s

go 1.15

require (
	github.com/kata-containers/tests/metrics/env v0.0.0-00010101000000-000000000000
	github.com/kata-containers/tests/metrics/exec v0.0.0-00010101000000-000000000000
	github.com/kata-containers/tests/metrics/k8s v0.0.0-00010101000000-000000000000
	github.com/pkg/errors v0.9.1
	github.com/sirupsen/logrus v1.8.1
	github.com/urfave/cli v1.22.5
)

replace github.com/kata-containers/tests/metrics/exec => ../../pkg/exec

replace github.com/kata-containers/tests/metrics/k8s => ../../pkg/k8s

replace github.com/kata-containers/tests/metrics/env => ../../pkg/env
