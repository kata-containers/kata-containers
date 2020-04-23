// Copyright (c) 2016 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

/*
Package virtcontainers manages hardware virtualized containers.
Each container belongs to a set of containers sharing the same networking
namespace and storage, also known as a sandbox.

Virtcontainers sandboxes are hardware virtualized, i.e. they run on virtual machines.
Virtcontainers will create one VM per sandbox, and containers will be created as
processes within the sandbox VM.

The virtcontainers package manages both sandboxes and containers lifecycles.
*/
package virtcontainers
