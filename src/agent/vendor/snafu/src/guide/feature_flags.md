# Optional extensions to the crate

In addition to the feature flags [controlling compatibility],
there are Cargo [feature flags] that extend SNAFU for various use
cases:

- [`std`](#std)
- [`unstable-core-error`](#unstable-core-error)
- [`guide`](#guide)
- [`backtraces`](#backtraces)
- [`backtraces-impl-backtrace-crate`](#backtraces-impl-backtrace-crate)
- [`backtraces-impl-std`](#backtraces-impl-std)
- [`unstable-backtraces-impl-std`](#unstable-backtraces-impl-std)
- [`unstable-provider-api`](#unstable-provider-api)
- [`futures`](#futures)
- [`unstable-try-trait`](#unstable-try-trait)

[controlling compatibility]: super::guide::compatibility
[feature flags]: https://doc.rust-lang.org/stable/cargo/reference/specifying-dependencies.html#choosing-features

<style>
.snafu-ff-meta>dt {
  font-weight: bold;
}
.snafu-ff-meta>*>p {
  margin: 0;
}
</style>

## `std`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>enabled</dd>
</dl>

When enabled, SNAFU will implement the `std::error::Error` trait. When
disabled, SNAFU will instead implement a custom `Error` trait that is
similar, but does not need any features from the standard library.

See also [`unstable-core-error`](#unstable-core-error).

Most usages of SNAFU will want this feature enabled.

## `unstable-core-error`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
</dl>

When enabled, SNAFU will implement the `core::error::Error` trait,
even when the `std` feature flag is also enabled.

## `guide`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
</dl>

When enabled, the `guide` module containing the user's guide will be
built.

Most usages of SNAFU will want this feature disabled.

## `backtraces`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
<dt>Implies</dt>
<dd>

[`std`](#std)

</dd>
</dl>
</dl>

When enabled, the [`Backtrace`] type in your enum variant will capture
a backtrace when the error is generated. If you never use backtraces,
you can omit this feature to speed up compilation a small amount.

It is recommended that only applications make use of this feature.

[`Backtrace`]: crate::Backtrace

## `backtraces-impl-backtrace-crate`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
<dt>Implies</dt>
<dd>

[`backtraces`](#backtraces)

</dd>
</dl>

When enabled, the SNAFU [`Backtrace`] type becomes an alias to the
`backtrace::Backtrace` type. This allows interoperability with other
crates that require this type.

It is recommended that only applications make use of this
feature. When the standard library stabilizes its own backtrace type,
this feature will no longer be supported and will be removed.

## `backtraces-impl-std`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
</dl>

When enabled, the SNAFU [`Backtrace`] type becomes an alias to the
[`std::backtrace::Backtrace`] type.

It is recommended that only applications make use of this feature.

## `unstable-backtraces-impl-std`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
<dt>Implies</dt>
<dd>

[`backtraces-impl-std`](#backtraces-impl-std)

</dd>
</dl>

When enabled, the `std::error::Error::backtrace` method is implemented.

It is recommended that only applications make use of this feature.

## `unstable-provider-api`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
</dl>

When enabled, SNAFU-generated errors will implement the
[`std::error::Error::provide`] method, allowing data to be retrieved
using `request_ref` and `request_value` on a [`std::error::Error`]
trait object reference. Provided data can be controlled using
[`#[snafu(provide)]`][snafu-provide].

It is recommended that only applications make use of this feature.

[snafu-provide]: crate::Snafu#providing-data-beyond-the-error-trait

## `futures`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>disabled</dd>
</dl>

When enabled, you can use the [`futures::TryFutureExt`] and
[`futures::TryStreamExt`] traits to add context methods to futures
and streams returning `Result`s.

[`futures::TryFutureExt`]: crate::futures::TryFutureExt
[`futures::TryStreamExt`]: crate::futures::TryStreamExt

## `unstable-try-trait`

**default**: disabled

When enabled, the `?` operator can be used on [`Result`][] values in
functions where a [`Report`][] type is returned.

It is recommended that only applications make use of this feature.

[`Report`]: crate::Report
