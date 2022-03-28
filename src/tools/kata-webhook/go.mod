module github.com/kata-containers/kata-containers/src/tools/kata-webhook

go 1.17

require (
	github.com/sirupsen/logrus v1.8.1
	github.com/slok/kubewebhook/v2 v2.3.0
	k8s.io/api v0.23.5
	k8s.io/apimachinery v0.23.5
)

require (
	github.com/google/gofuzz v1.2.0 // indirect
	golang.org/x/net v0.0.0-20220127200216-cd36cc0744dd // indirect
	golang.org/x/sys v0.0.0-20220204135822-1c1b9b1eba6a // indirect
)

require (
	github.com/go-logr/logr v1.2.2 // indirect
	github.com/gogo/protobuf v1.3.2 // indirect
	github.com/google/go-cmp v0.5.7 // indirect
	github.com/json-iterator/go v1.1.12 // indirect
	github.com/kata-containers/kata-containers/src/runtime v0.0.0-20220325211203-a07956a369ab
	github.com/modern-go/concurrent v0.0.0-20180306012644-bacd9c7ef1dd // indirect
	github.com/modern-go/reflect2 v1.0.2 // indirect
	golang.org/x/text v0.3.7 // indirect
	gomodules.xyz/jsonpatch/v3 v3.0.1 // indirect
	gomodules.xyz/orderedmap v0.1.0 // indirect
	gopkg.in/inf.v0 v0.9.1 // indirect
	gopkg.in/yaml.v2 v2.4.0 // indirect
	k8s.io/client-go v0.23.5 // indirect
	k8s.io/klog/v2 v2.30.0 // indirect
	k8s.io/utils v0.0.0-20211116205334-6203023598ed // indirect
	sigs.k8s.io/json v0.0.0-20211020170558-c049b76a60c6 // indirect
	sigs.k8s.io/structured-merge-diff/v4 v4.2.1 // indirect
	sigs.k8s.io/yaml v1.2.0 // indirect
)

replace github.com/kata-containers/kata-containers/src/runtime => ../../runtime
