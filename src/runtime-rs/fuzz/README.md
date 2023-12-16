# Fuzz testing
To get started with fuzz testing you'll need to install cargo-fuzz.

`cargo install cargo-fuzz`

You can list the available fuzz harnesses using cargo fuzz. e.g.
```
$ cd src/runtime-rs/fuzz
$ cargo +nightly fuzz list
fuzz_hypervisor_device_roundtrip
```

To run a fuzz harness simply use the following command.

```
$ cargo +nightly run fuzz_hypervisor_device_roundtrip
```

The fuzzer will run until;
- It finds a bug and crashes.
- You manually exit fuzzing i.e. 'ctrl-C'.
- You manually set a timeout.

For more complete documentation on cargo-fuzz see the 
[book](https://rust-fuzz.github.io/book/introduction.html).
