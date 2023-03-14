## Rust version compatibility

SNAFU is tested and compatible back to Rust 1.34, released on
2019-05-14. Compatibility is controlled by Cargo feature flags.

<style>
.snafu-ff-meta>dt {
  font-weight: bold;
}
.snafu-ff-meta>*>p {
  margin: 0;
}
</style>

## `rust_1_46`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>enabled</dd>
</dl>

When enabled, SNAFU will assume that it's safe to target features
available in Rust 1.46. Notably, the `#[track_caller]` feature is
needed to allow [`Location`][crate::Location] to automatically discern
the source code location.

## `rust_1_61`

<dl class="snafu-ff-meta">
<dt>Default</dt>
<dd>enabled</dd>
<dt>Implies</dt>
<dd>

[`rust_1_46`](#rust_1_46)

</dd>
</dl>

When enabled, SNAFU will assume that it's safe to target features
available in Rust 1.61. Notably, the [`Termination`][] trait is
implemented for [`Report`][] to allow it to be returned from `main`
and test functions.

[`Termination`]: std::process::Termination
[`Report`]: crate::Report
