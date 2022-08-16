The `src/libs` directory hosts library crates which may be shared by multiple Kata Containers components
or published to [`crates.io`](https://crates.io/index.html).

### Library Crates
Currently it provides following library crates:

| Library | Description |
|-|-|
| [logging](logging/) | Facilities to setup logging subsystem based on slog. |
| [system utilities](kata-sys-util/) | Collection of facilities and helpers to access system services. |
| [types](kata-types/) | Collection of constants and data types shared by multiple Kata Containers components. |
| [safe-path](safe-path/) | Utilities to safely resolve filesystem paths. |
| [test utilities](test-utils/) | Utilities to share test code. |
