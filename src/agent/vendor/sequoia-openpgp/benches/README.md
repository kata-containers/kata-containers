# Benchmarks

We use [`criterion`](https://crates.io/crates/criterion) as a benchmark framework. It is
* statistics driven,
* configurable,
* produces nice plots
* and is compatible with stable Rust.

### Usage

To compare your work to `main`:

```
git switch main
cargo bench -- --save-baseline main
git switch branchname
cargo bench --baseline main
```
The html report can then be found at `sequoia/target/criterion/report/index.html`.

You can also create a report for two stored baselines without running the
benchmarks again:
```
cargo bench --load-baseline my_baseline --baseline main
```
#### Critmp

Criterion can only include up to two baselines in one report.
If you'd like to compare more than two stored baselines, or see a report on the
command line, use [`critcmp`], e.g.
```
critcmp my_baseline my_other_baseline main
```

[`critcmp`]: https://crates.io/crates/critcmp

#### Useful commands

To run the benchmarks:
```
cargo bench
```

To run a specific benchmark:
```
cargo bench -- benchmark_name
```

To test the benchmarks:
```
cargo test --benches
```

To test a specific benchmark:
```
cargo test --benches -- benchmark_name
```
