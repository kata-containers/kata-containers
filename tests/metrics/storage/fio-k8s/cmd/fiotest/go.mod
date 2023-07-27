module github.com/kata-containers/kata-containers/tests/metrics/storage/fio-k8s

go 1.19

replace github.com/kata-containers/kata-containers/tests/metrics/exec => ../../pkg/exec

replace github.com/kata-containers/kata-containers/tests/metrics/k8s => ../../pkg/k8s

replace github.com/kata-containers/kata-containers/tests/metrics/env => ../../pkg/env

require (
	github.com/kata-containers/kata-containers/tests/metrics/env v0.0.0-00010101000000-000000000000
	github.com/kata-containers/kata-containers/tests/metrics/exec v0.0.0-00010101000000-000000000000
	github.com/kata-containers/kata-containers/tests/metrics/k8s v0.0.0-00010101000000-000000000000
	github.com/pkg/errors v0.9.1
	github.com/sirupsen/logrus v1.9.3
	github.com/urfave/cli v1.22.14
)

require (
	github.com/cpuguy83/go-md2man/v2 v2.0.2 // indirect
	github.com/russross/blackfriday/v2 v2.1.0 // indirect
	golang.org/x/sys v0.0.0-20220715151400-c0bba94af5f8 // indirect
)
