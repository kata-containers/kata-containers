# How to monitor Kata Containers in Kubernetes clusters

This document describes how to run `kata-monitor` in a Kubernetes cluster using Prometheus's service discovery to scrape metrics from `kata-agent`.

> **Warning**: This how-to is only for evaluation purpose, you **SHOULD NOT** running it in production using this configurations.

## Introduction

If you are running Kata containers in a Kubernetes cluster, the best way to run `kata-monitor` is using Kubernetes native `DaemonSet`, `kata-monitor` will run on desired Kubernetes nodes without other operations when new nodes joined the cluster.

Prometheus also support a Kubernetes service discovery that can find scrape targets dynamically without explicitly setting `kata-monitor`'s metric endpoints.

## Pre-requisites

You must have a running Kubernetes cluster first. If not, [install a Kubernetes cluster](https://kubernetes.io/docs/setup/) first.

Also you should ensure that `kubectl` working correctly.

> **Note**: More information about Kubernetes integrations:
>   - [Run Kata Containers with Kubernetes](run-kata-with-k8s.md)
>   - [How to use Kata Containers and Containerd](containerd-kata.md)
>   - [How to use Kata Containers and containerd with Kubernetes](how-to-use-k8s-with-containerd-and-kata.md)

## Configure Prometheus

Start Prometheus by utilizing our sample manifest:

```
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/docs/how-to/data/prometheus.yml
```

This will create a new namespace, `prometheus`, and create the following resources:

* `ClusterRole`, `ServiceAccount`, `ClusterRoleBinding` to let Prometheus to access Kubernetes API server.
* `ConfigMap` that contains minimum configurations to let Prometheus run Kubernetes service discovery.
* `Deployment` that run Prometheus in `Pod`.
* `Service` with `type` of `NodePort`(`30909` in this how to), that we can access Prometheus through `<hostIP>:30909`. In production environment, this `type` may be `LoadBalancer` or `Ingress` resource.

After the Prometheus server is running, run `curl -s http://hostIP:NodePort:30909/metrics`, if Prometheus is working correctly, you will get response like these:

```
# HELP go_gc_duration_seconds A summary of the GC invocation durations.
# TYPE go_gc_duration_seconds summary
go_gc_duration_seconds{quantile="0"} 3.9403e-05
go_gc_duration_seconds{quantile="0.25"} 0.000169907
go_gc_duration_seconds{quantile="0.5"} 0.000207421
go_gc_duration_seconds{quantile="0.75"} 0.000229911
```

## Configure `kata-monitor`

`kata-monitor` can be started on the cluster as follows:

```
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/docs/how-to/data/kata-monitor-daemonset.yml
```

This will create a new namespace `kata-system` and a `daemonset` in it.

Once the `daemonset` is running, Prometheus should discover `kata-monitor` as a target. You can open `http://<hostIP>:30909/service-discovery` and find `kubernetes-pods` under the `Service Discovery` list


## Setup Grafana

Run this command to run Grafana in Kubernetes:

```
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/docs/how-to/data/grafana.yml
```

This will create deployment and service for Grafana under namespace `prometheus`.

After the Grafana deployment is ready, you can open `http://hostIP:NodePort:30000/` to access Grafana server. For Grafana 7.0.5, the default user/password is `admin/admin`. You can modify the default account and adjust other security settings by editing the [Grafana configuration](https://grafana.com/docs/grafana/latest/installation/configuration/#security).

To use Grafana show data from Prometheus, you must create a Prometheus `datasource` and dashboard.

### Create `datasource`

Open `http://hostIP:NodePort:30000/datasources/new` in your browser, select Prometheus from time series databases list.

Normally you only need to set `URL` to `http://hostIP:NodePort:30909` to let it work, and leave the name as `Prometheus` as default.

### Import dashboard

A [sample dashboard](data/dashboard.json) for Kata Containers metrics is provided which can be imported to Grafana for evaluation.

You can import this dashboard using Grafana UI, or using `curl` command in console.


```
$ curl -XPOST -i localhost:3000/api/dashboards/import \
    -u admin:admin \
    -H "Content-Type: application/json" \
	-d "{\"dashboard\":$(curl -sL https://raw.githubusercontent.com/kata-containers/kata-containers/main/docs/how-to/data/dashboard.json )}"
```

## References

- [Prometheus `kubernetes_sd_config`](https://prometheus.io/docs/prometheus/latest/configuration/configuration/#kubernetes_sd_config)

