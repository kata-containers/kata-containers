prctl
=====

Rust library providing the prctl abstraction

Module provides safe abstraction over the prctl interface.
Provided functions map to a single `prctl()` call, although some of them may be usable
only on a specific architecture or only with root privileges. All known enums that
may be used as parameters are provided in this crate.

Each function provides result which will be `Err(errno)` in case the `prctl()` call fails.

To run tests requiring root privileges, enable feature "root_test".

Usage
=====

Most functions set/get flags or set/get options. They can be used in the following way:
```
// Allow core dumping
!try(prctl::set_dumpable(true));

// Get current timer slack
let slack = !try(prctl::get_timer_slack());

// Send signal 6 after dying
!try(prctl::set_death_signal(6));

// Set current process name
!try(prctl::set_name("new_process"));

// Disable access to the timestamp counter
use prctl::PrctlTsc;
!try(prctl::set_tsc(PrctlTsc::PR_TSC_SIGSEGV));
```
