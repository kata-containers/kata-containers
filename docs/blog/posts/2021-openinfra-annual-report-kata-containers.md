---
templateKey: blog-post
title: "2021 OpenInfra Annual Report: Kata Containers"
author: Kata Containers
date: 2022-01-26
---

![](/img/1_4BSHK0mPTgBUJFjkaCgaOg.webp)

Kata Containers continues to deliver the speed of containers with the security of virtual machines. Kata Containers became a pilot project in December 2017, in conjunction with the Open Infrastructure Foundation’s evolution from being the home for OpenStack to becoming the home of open infrastructure collaboration. In April of 2019, Kata Containers was the first Open Infrastructure Foundation pilot project to graduate, becoming an official open infrastructure project. In 2021, Kata Containers continued to expand its use cases and gain more users.

<!-- more -->

Kata Containers started by providing stronger isolation than traditional containers. In 2021, the Kata Containers community expanded Kata Containers’ usage scenario from security isolation to also cover performance isolation. It prevents different workloads from affecting each other in both security and performance aspects. Kata Containers is also supporting confidential containers use case with TDX/SEV/IBM SE enabled isolation, further expanding Kata Containers’ threat model from protecting the infrastructure to also protecting the workloads.

In 2021, the Kata Containers community continues to present its strategic relevance, well-defined governance procedures, commitment to technical best practices and open collaboration, and, most importantly, an actively engaged ecosystem of developers and users.

The project continues to cultivate a global, engaged, and growing community as evidenced by the 2021 stats: 23 major/stable releases with 1,400 plus commits made by 95 authors from 16 organizations. The top five contributing companies include Intel, Red Hat, Ant Group, IBM, and Apple.

Kata Containers production deployments continued to expand in 2021, for example:

*   In recent years, Kata Containers helped Ant Group build up the sustainable IT infrastructure. Thanks to the strong isolation of Kata Containers, Ant Group could deploy online applications and batch jobs together on thousands of nodes without significant interference. As a result of adopting Kata Containers and other related technologies, Ant Group reduced half of the per-payment energy consumption in the recent Double-Eleven e-Shopping Festival compared to three years ago. Part of the deployment has been upgraded to a 3.0 pre-release version of the integrated rust shim and sandbox, which is open-sourced and being reviewed by the community as a major feature of Kata 3.0.
*   Red Hat provides Kata Containers as a secondary container runtime for OpenShift Clusters with OpenShift sandboxed containers. OpenShift sandboxed containers use the [Kubernetes Resource Model (KRM](https://kubernetes.io/docs/tasks/manage-kubernetes-objects/declarative-config/)) to declaratively configure and customize the entire Kata Containers deployment and day 2 customizations. The end result is additional isolation with the same cloud-native user experience.
*   The Kata Containers project’s continued innovation and deep community expertise is why IBM believes it to be the best solution for strongly isolating customer’s production CI/CD workloads.
*   Exotanium leverages the Kata Containers architecture to implement a new cloud resource management technology with the capability of performing transparent live migration of containers. With this technology, Exotanium X-Spot offers a new solution of running stateful, long-running workloads in AWS EC2 Spot instances and relocating containers before they are terminated.
*   Nubificus builds on Kata Containers to deliver an interoperable serverless framework for cloud and edge resources that enables user functions to use hardware acceleration without direct access to the hardware.

The Kata Containers Architecture Committee also went through two election cycles: one in February and another one in September, 2021. Now, the project is led by Archana Shinde (Intel), Eric Ernst (Apple), Fabiano Fidêncio (Intel), Samuel Ortiz (Apple), and Tao Peng (Ant Group). The Architecture Committee members ensure that Kata Containers continues to be aligned with its goal of open collaboration and innovation around container speed and security.

Looking ahead to 2022, Kata Containers is evolving with a new rust-based runtime with integrated sandbox. It will further reduce Kata Containers memory footprint as well as installation and management complexity. Besides the new rust-based runtime, the Kata Containers community will continue putting the focus on improving its integration to the cloud native ecosystem, and supporting the confidential container use case. The community is planning a new major release 3.0 targeting the middle of 2022.

The Kata Containers’ project code is hosted on Github under the Apache 2 license. Learn about Kata Containers, how to contribute and support the community at katacontainers.io. Join these channels to get involved:

*   Code: [github.com/kata-containers](https://github.com/kata-containers)
*   Slack: [katacontainers.slack.com](https://bit.ly/kataslack)
*   IRC: [#kata-dev](http://webchat.oftc.net/?channels=kata-dev) on OFTC
*   Mailing lists: [lists.katacontainers.io](https://lists.katacontainers.io/)
*   Website: [katacontainers.io](https://katacontainers.io/)

You can read the full 2021 OpenInfra Annual Report [here](https://openinfra.dev/annual-report/2021).