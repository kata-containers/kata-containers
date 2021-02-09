# Patterns & Guidance

failure is not a "one size fits all" approach to error management. There are
multiple patterns that emerge from the API this library provides, and users
need to determine which pattern makes sense for them. This section documents
some patterns and how users might use them.

In brief, these are the patterns documented here:

- **[Strings as errors](./error-msg.md):** Using strings as your error
  type. Good for prototyping.
- **[A Custom Fail type](./custom-fail.md):** Defining a custom type to be
  your error type. Good for APIs where you control all or more of the
  possible failures.
- **[Using the Error type](./use-error.md):** Using the Error type to pull
  together multiple failures of different types. Good for applications and
  APIs that know the error won't be inspected much more.
- **[An Error and ErrorKind pair](./error-errorkind.md):** Using both a
  custom error type and an ErrorKind enum to create a very robust error
  type. Good for public APIs in large crates.

(Though each of these items identifies a use case which this pattern would be
good for, in truth each of them can be applied in various contexts. Its up to
you to decide what makes the most sense for your particular use case.)
