![Build status](https://github.com/antifuchs/governor/actions/workflows/ci_push.yml/badge.svg?branch=master) [![codecov](https://codecov.io/gh/antifuchs/governor/branch/master/graph/badge.svg)](https://codecov.io/gh/antifuchs/governor) [![Docs](https://docs.rs/governor/badge.svg)](https://docs.rs/governor/) [![crates.io](https://img.shields.io/crates/v/governor.svg)](https://crates.io/crates/governor)

# governor - a library for regulating the flow of data

This library is an implementation of the [Generic Cell Rate
Algorithm](https://en.wikipedia.org/wiki/Generic_cell_rate_algorithm)
for rate limiting in Rust programs.

It is intended to help your program know how much strain it is
supposed to put on external services (and, to some extent, to allow
your services to regulate how much strain they take on from their
users). In a way, it functions like the iconic steam governor from
which this library takes its name:

![a centrifugal governor](doc/centrifugal-governor.png)

## Related projects

 + [tide-governor](https://github.com/ohmree/tide-governor): A tide middleware that provides rate-limiting functionality backed by governor.

 + [actix-governor](https://github.com/AaronErhardt/actix-governor): A middleware for actix-web that provides rate-limiting backed by governor.

## Implementation and constraints

The rate-limiting algorithms in this crate are implemented using the
Generic Cell Rate Algorithm (GCRA). The GCRA is functionally
equivalent to a leaky bucket, but has a few advantages over most
leaky bucket implementations:

* No background "drip" process is necessary to keep up maintenance on
  the bucket,
* it updates its state, whenever a request comes in, continuously on a
  nanosecond scale, and
* it keeps its state in a single `AtomicU64` integer.

The rate-limiting state used here does not take up much memory (only
64 bits!) and is updated thread-safely with a compare-and-swap
operation. Compared to `ratelimit_meter`'s implementation using
Mutexes, it is on average 10x faster when used on multiple threads.

### Constraints

The speed comes at a slight cost: Each rate-limiter and its state is
only useful for 584 years after creation. If you are trying to power
the [Long Now Foundation](http://longnow.org/)'s computers with this
library, please get in touch.

## How does this relate to [`ratelimit_meter`](https://github.com/antifuchs/ratelimit_meter)?

This project is a fork/rebranding/continuation of `ratelimit_futures`,
based on a few key insights and advancements in the ecosystem:

* The 2018 edition is now both available and extremely useful.
* Futures and `async`/`await` are stable.
* `ratelimit_meter` was too generic for its own good, implementing two
  suboptimal variants of the same rate-limiting algorithm.

Let's go through these in order:

### Rust 2018

The code in this crate targets Rust's 2018 edition. This has allowed
making the code less complicated and more idiomatic.

### `async`/`await`

Before Rust 1.39, the only way to use Futures in stable was to make
combinators or poll them manually. There is a crate for
`ratelimit_meter` that [implements a rate-limiting
future](https://github.com/antifuchs/ratelimit_futures/blob/ea83c1ae468e6089529ce24224686c27c85e5706/src/lib.rs#L70-L155)
in about 80 lines. The equivalent functionality with `async`/`await`
takes about three lines in this crate.

### No more two algorithms that do the same

`ratelimit_meter` shipped "two" algorithm classes, `LeakyBucket` and
`GCRA`. These behaved exactly the same (modulo a glitch where the
first cell in a GCRA was free), forcing every user to make a decision
that ultimately meant nothing.

This crate implements only the GCRA algorithm (minus the "first cell
is free" glitch) and does so in a more optimal way than
`ratelimit_meter` could have.

The return values here are mostly concrete types, and the only type
parameter most things accept is the clock implementation, in order to
compile on `no_std`.

### So, why make a new crate?

There are a few reasons, but mostly these: One, I was unhappy with the
name, which I found to not fit very well anymore; and two, I felt such
a radically new interface would be burdensome on users in addition to
being hard to implement incrementally in the old repo. These reasons
may not be very good, but here we are.
