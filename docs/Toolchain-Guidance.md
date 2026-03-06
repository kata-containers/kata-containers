# Toolchains

As a community we want to strike a balance between having up-to-date toolchains, to receive the
latest security fixes and to be able to benefit from new features and packages, whilst not being
too bleeding edge and disrupting downstream and other consumers. As a result we have the following
guidelines (note, not hard rules) for our go and rust toolchains that we are attempting to try out:

## Go toolchain

Go is released [every six months](https://go.dev/wiki/Go-Release-Cycle) with support for the
[last two major release versions](https://go.dev/doc/devel/release#policy). We always want to
ensure that we are on a supported version so we receive security fixes. To try and make
things easier for some of our users, we aim to be using the older of the two supported major
versions, unless there is a compelling reason to adopt the newer version.

In practice this means that we bump our major version of the go toolchain every six months to
version (1.x-1) in response to a new version (1.x) coming out, which makes our current version
(1.x-2) no longer supported. We will bump the minor version whenever required to satisfy
dependency updates, or security fixes.

Our go toolchain version is recorded in [`versions.yaml`](../versions.yaml) under
`.languages.golang.version` and should match with the version in our `go.mod` files.

## Rust toolchain

Rust has a [six week](https://doc.rust-lang.org/book/appendix-05-editions.html#:~:text=The%20Rust%20language%20and%20compiler,these%20tiny%20changes%20add%20up.)
release cycle and they only support the latest stable release, so if we wanted to remain on a
supported release we would only ever build with the latest stable and bump every 6 weeks.
However feedback from our community has indicated that this is a challenge as downstream consumers
often want to get rust from their distro, or downstream fork and these struggle to keep up with
the six week release schedule. As a result the community has agreed to try out a policy of
"stable-2", where we aim to build with a rust version that is two versions behind the latest stable
version.

In practice this should mean that we bump our rust toolchain every six weeks, to version
1.x-2 when 1.x is released as stable and we should be picking up the latest point release
of that version, if there were any.

The rust-toolchain that we are using is recorded in [`rust-toolchain.toml`](../rust-toolchain.toml).

> [!NOTE]
> We don't currently have a firm policy on the minimum supported rust version (MSRV) for our components
> and we generally don't have this listed explicitly (via `rust-version`) in our crates.
> When bumping the toolchain version we attempt to stay up-to-date with new language features that
> improve the current codebase (e.g. fixing new clipping warnings) and this often will result in
> the MSRV being implicitly bumped so we can use these features, but we don't have a hard requirement
> that bumps the MSRV to the same level as the toolchain.
