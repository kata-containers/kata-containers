# Contributing to micro-http

## Contribution Workflow

The micro-http repository uses the “fork-and-pull” development model. Follow
these steps if you want to merge your changes:

1. Within your fork of
   [micro-http](https://github.com/firecracker-microvm/micro-http), create a
   branch for your contribution. Use a meaningful name.
1. Create your contribution, meeting all
   [contribution quality standards](#contribution-quality-standards)
1. [Create a pull request](https://help.github.com/articles/creating-a-pull-request-from-a-fork/)
   against the master branch of the micro-http repository.
1. Work with your reviewers to address any comments and obtain a
   minimum of 2 approvals, at least one of which must be provided by
   [a maintainer](MAINTAINERS.md).
   To update your pull request amend existing commits whenever applicable and
   then push the new changes to your pull request branch.
1. Once the pull request is approved, one of the maintainers will merge it.

## Request for Comments

If you just want to receive feedback for a contribution proposal, open an “RFC”
(“Request for Comments”) pull request:

1. On your fork of
   [micro-http](https://github.com/firecracker-microvm/micro-http), create a
   branch for the contribution you want feedback on. Use a meaningful name.
1. Create your proposal based on the existing codebase.
1. [Create a draft pull request](https://github.blog/2019-02-14-introducing-draft-pull-requests/)
   against the master branch of the micro-http repository.
1. Discuss your proposal with the community on the pull request page (or on any
   other channel). Add the conclusion(s) of this discussion to the pull request
   page.

## Contribution Quality Standards

Most quality and style standards are enforced automatically during integration
testing. Your contribution needs to meet the following standards:

- Separate each **logical change** into its own commit.
- Each commit must pass all unit & code style tests, and the full pull request
  must pass all integration tests.
- Unit test coverage must _increase_ the overall project code coverage.
- Document all your public functions.
- Add a descriptive message for each commit. Follow
  [commit message best practices](https://github.com/erlang/otp/wiki/writing-good-commit-messages).
- Document your pull requests. Include the reasoning behind each change.
- Acknowledge micro-http's [Apache 2.0 license](LICENSE) and certify that no
  part of your contribution contravenes this license by signing off on all your
  commits with `git -s`. Ensure that every file in your pull request has a
  header referring to the repository license file.
