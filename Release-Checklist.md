# Release Checklist

This document lists the tasks required to create a Kata Release.

It should be pasted directly into a GitHub issue and each item checked off as it is completed.

- [ ] Raise PRs to update the `VERSION` file in the following repositories:
   - [ ] [agent][agent]
   - [ ] [proxy][proxy]
   - [ ] [runtime][runtime]
   - [ ] [shim][shim]
   - [ ] [throttler][throttler]

  Note that the "phase" element of the project encoded in the version strings needs to match for all components. For example, when a `beta` is released, the version string for *all* components should show `beta`.

- [ ] Ensure all CI tests pass **for all architectures**.

- [ ] Get confirmation from metrics CI that performance is within acceptable limits.

- [ ] Create a **signed and annotated tag** for the new release version for the following repositories:
   - [ ] [agent][agent]
   - [ ] [proxy][proxy]
   - [ ] [runtime][runtime]
   - [ ] [shim][shim]
   - [ ] [throttler][throttler]

   This is required by `git describe`.

- [ ] Generate OBS packages based on `HEAD`:
   - [ ] [agent][agent]
   - [ ] [guest kernel][kernel]
   - [ ] [image][image]
   - [ ] [initrd][initrd]
   - [ ] [proxy][proxy]
   - [ ] [`qemu-lite`][qemu-lite]
   - [ ] [runtime][runtime]
   - [ ] [shim][shim]
   - [ ] [throttler][throttler]

- [ ] Generate snap packages based on `HEAD`
   - [ ] Push snap packages via snapcraft tool
   - [ ] Publish snap packages in the snapcraft store

- [ ] Installation tests (must be done for major releases):
  - [ ] CentOS
  - [ ] Fedora
  - [ ] Ubuntu

- [ ] Check if any of the following need to be updated:
  - [ ] Architecture document
  - [ ] [Developer guide](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md)
  - [ ] Installation documentation
  - [ ] [Limitations document](https://github.com/kata-containers/documentation/blob/master/Limitations.md)

- [ ] Write release notes:
  - [ ] Link to Limitations document.
  - [ ] Brief summary of known issues (with links to appropriate Issues/PRs) for any late-breaking issues.
  - [ ] List new features.
  - [ ] List resolved bugs and limitations.
  - [ ] Version of Docker (ideally range of versions, or "up to version X") supported by the release.
  - [ ] CRI-O version.
  - [ ] `cri-containerd` version.
  - [ ] Version of the OCI spec (ideally range of versions, or "up to version X") supported by the release.
  - [ ] Version of image used by the release (guest kernel version, guest O/S version, and agent version).
  - [ ] Add links to Installation instructions.
  - [ ] Document any common vulnerabilities and exposures (CVEs) fixed with links to the CVE database.

- [ ] Post release details on the public mailing list and Slack.

- [ ] Update public IRC channel with a link to the latest release.

- [ ] Arrange communication of the release through other social media channels.

[agent]: https://github.com/kata-containers/agent
[image]: https://github.com/kata-containers/osbuilder/tree/master/image-builder
[initrd]: https://github.com/kata-containers/osbuilder/tree/master/initrd-builder
[kernel]: https://github.com/kata-containers/linux
[proxy]: https://github.com/kata-containers/proxy
[qemu-lite]: https://github.com/kata-containers/qemu
[runtime]: https://github.com/kata-containers/runtime
[shim]: https://github.com/kata-containers/shim
[tests]: https://github.com/kata-containers/tests
[throttler]: https://github.com/kata-containers/ksm-throttler
