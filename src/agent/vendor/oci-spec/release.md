# Releasing a new version of oci-spec-rs

A new release of this crate can be proposed by running the version bump script:

```console
./hack/release x.y.z
```

Push the changes to your fork and draft a new GitHub Pull Request (PR) which
should now contain 2 commits, one which bumps the release version and another
one to turn it _back to dev_.

If the PR got merged, then create a new tag pointing to the first commit of that
PR (named _Bump to x.y.z_). The changelog can be created by using GitHub's
release note creation feature via the user interface.
