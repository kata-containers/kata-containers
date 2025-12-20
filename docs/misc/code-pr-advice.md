# Code PR Advice

Before raising a PR containing code changes, we suggest you consider
the following to ensure a smooth and fast process.

> **Note:**
>
> - All the advice in this document is optional. However, if the
>   advice provided is not followed, there is no guarantee your PR
>   will be merged.
>
> - All the check tools will be run automatically on your PR by the CI.
>   However, if you run them locally first, there is a much better
>   chance of a successful initial CI run.

## Assumptions

This document assumes you have already read (and in the case of the
code of conduct agreed to):

- The [Kata Containers code of conduct](https://github.com/kata-containers/community/blob/main/CODE_OF_CONDUCT.md).
- The [Kata Containers contributing guide](https://github.com/kata-containers/community/blob/main/CONTRIBUTING.md).

## Code

### Architectures

Do not write architecture-specific code if it is possible to write the
code generically.

### General advice

- Do not write code to impress: instead write code that is easy to read and understand.

- Always consider which user will run the code. Try to minimise
  the privileges the code requires.

### Comments

Always add comments if the intent of the code is not obvious. However,
try to avoid comments if the code could be made clearer (for example
by using more meaningful variable names).

### Constants

Don't embed magic numbers and strings in functions, particularly if
they are used repeatedly.

Create constants at the top of the file instead.

### Copyright and license

Ensure all new files contain a copyright statement and an SPDX license
identifier in the comments at the top of the file.

### FIXME and TODO

If the code contains areas that are not fully implemented, make this
clear a comment which provides a link to a GitHub issue that provides
further information.

Do not just rely on comments in this case though: if possible, return
a "`BUG: feature X not implemented see {bug-url}`" type error.

### Functions

- Keep functions relatively short (less than 100 lines is a good "rule of thumb").

- Document functions if the parameters, return value or general intent
  of the function is not obvious.

- Always return errors where possible.

  Do not discard error return values from the functions this function
  calls.

### Logging

- Don't use multiple log calls when a single log call could be used.

- Use structured logging where possible to allow
  [standard tooling](../src/tools/log-parser)
  be able to extract the log fields.

### Names

Give functions, macros and variables clear and meaningful names.

### Structures

#### Golang structures

Unlike Rust, Go does not enforce that all structure members be set.
This has lead to numerous bugs in the past where code like the
following is used:

```go
type Foo struct {
    Key   string
    Value string
}

// BUG: Key not set, but nobody noticed! ;(
let foo1 = Foo {
    Value: "foo",
}
```

A much safer approach is to create a constructor function to enforce
integrity:

```go
type Foo struct {
    Key   string
    Value string
}

func NewFoo(key, value string) (*Foo, error) {
    if key == "" {
        return nil, errors.New("Foo needs a key")
    }

    if value == "" {
        return nil, errors.New("Foo needs a value")
    }

    return &Foo{
        Key:   key,
        Value: value,
    }, nil
}

func testFoo() error {
    // BUG: Key not set, but nobody noticed! ;(
    badFoo := Foo{Value: "value"}

    // Ok - the constructor performs needed validation
    goodFoo, err := NewFoo("name", "value")
    if err != nil {
        return err
    }

    return nil
```

> **Note:**
>
> The above is just an example. The *safest* approach would be to move
> `NewFoo()` into a separate package and make `Foo` and it's elements
> private. The compiler would then enforce the use of the constructor
> to guarantee correctly defined objects.


### Tracing

Consider if the code needs to create a new
[trace span](./tracing.md).

Ensure any new trace spans added to the code are completed.

## Tests

### Unit tests

Where possible, code changes should be accompanied by unit tests.

Consider using the standard
[table-based approach](Unit-Test-Advice.md)
as it encourages you to make functions small and simple, and also
allows you to think about what types of value to test.

### Other categories of test

Raised a GitHub issue in the Kata Containers repository that
explains what sort of test is required along with as much detail as
possible. Ensure the original issue is referenced in the issue.

### Unsafe code

#### Rust language specifics

Minimise the use of `unsafe` blocks in Rust code and since it is
potentially dangerous always write [unit tests][#unit-tests]
for this code where possible.

`expect()` and `unwrap()` will cause the code to panic on error.
Prefer to return a `Result` on error rather than using these calls to
allow the caller to deal with the error condition.

The table below lists the small number of cases where use of
`expect()` and `unwrap()` are permitted:

| Area | Rationale for permitting |
|-|-|
| In test code (the `tests` module) | Panics will cause the test to fail, which is desirable. |
| `lazy_static!()` | This magic macro cannot "return" a value as it runs before `main()`. |
| `defer!()` | Similar to golang's `defer()` but doesn't allow the use of `?`. |
| `tokio::spawn(async move {})` | Cannot currently return a `Result` from an `async move` closure. |
| If an explicit test is performed before the `unwrap()` / `expect()` | *"Just about acceptable"*, but not ideal `[*]` |
| `Mutex.lock()` | Almost unrecoverable if failed in the lock acquisition |


`[*]` - There can lead to bad *future* code: consider what would
happen if the explicit test gets dropped in the future. This is easier
to happen if the test and the extraction of the value are two separate
operations. In summary, this strategy can introduce an insidious
maintenance issue.

## Documentation

### General requirements

- All new features should be accompanied by documentation explaining:

  - What the new feature does

  - Why it is useful

  - How to use the feature

  - Any known issues or limitations

    Links should be provided to GitHub issues tracking the issues

- The [documentation requirements document](Documentation-Requirements.md)
  explains how the project formats documentation.

### Markdown syntax

Run the
[markdown checker](https://github.com/kata-containers/kata-containers/tree/main/tests/cmd/check-markdown)
on your documentation changes.

### Spell check

Run the
[spell checker](https://github.com/kata-containers/kata-containers/tree/main/tests/cmd/check-spelling)
on your documentation changes.

## Finally

You may wish to read the documentation that the
[Kata Review Team](https://github.com/kata-containers/community/blob/main/Rota-Process.md) use to help review PRs:

- [PR review guide](https://github.com/kata-containers/community/blob/main/PR-Review-Guide.md).
- [documentation review process](https://github.com/kata-containers/community/blob/main/Documentation-Review-Process.md).
