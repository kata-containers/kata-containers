# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- next-header -->

## [Unreleased] - ReleaseDate

# [0.2.3] - 2021-07-16
## Added
- Added `#[displaydoc("..")]` attribute for overriding a doc comment

# [0.2.2] - 2021-07-01
## Added
- Added prefix feature to use the doc comment from an enum and prepend it
  before the error message from each variant.

# [0.2.1] - 2021-03-26
## Added
- Added opt in support for ignoring extra doc attributes

# [0.2.0] - 2021-03-16
## Changed

- (BREAKING) disallow multiple `doc` attributes in display impl
  [https://github.com/yaahc/displaydoc/pull/22]. Allowing and ignoring extra
  doc attributes made it too easy to accidentally create a broken display
  implementation with missing context without realizing it, this change turns
  that into a hard error and directs users towards block comments if multiple
  lines are needed.

<!-- next-url -->
[Unreleased]: https://github.com/yaahc/displaydoc/compare/v0.2.3...HEAD
[0.2.3]: https://github.com/yaahc/displaydoc/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/yaahc/displaydoc/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/yaahc/displaydoc/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/yaahc/displaydoc/releases/tag/v0.2.0
