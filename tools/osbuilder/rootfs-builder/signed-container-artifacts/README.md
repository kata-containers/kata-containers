### Description

This directory provides some artifacts required for implementing and testing the kata-agent's ability to verify the signatures of container images pulled from the test `quay.io/kata-containers/confidential-containers` repository.

### Contents

It consists of:
- `signatures.tar` - a tar archive containing the signatures of `quay.io/kata-containers/confidential-containers:signed` and `quay.io/kata-containers/confidential-containers:other_signed`
- `public.gpg` - the public GPG key, paired to the private key pair that was used to sign `quay.io/kata-containers/confidential-containers:signed`
- `quay_policy.json` - a container policy file that allows insecure access to all repos except `quay.io/kata-containers`, in which it enforced signatures by the above key

### Usage

As part of the Confidential Containers v0 proof of concept these files will be built into the kata image and used for the purposes of testing verification of signed images see [Issue #2682](https://github.com/kata-containers/kata-containers/issues/2682). They are intended to be temporary whilst a better solution is found to pass them in, probably based on the attestation agent.