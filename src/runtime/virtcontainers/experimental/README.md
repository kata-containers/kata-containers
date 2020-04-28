# Experimental package description

* [What are "experimental" features?](#what-are-experimental-features)
* [What's the difference between "WIP" and "experimental"?](#whats-the-difference-between-wip-and-experimental)
* [When should "experimental" features be moved out from "experimental"?](#when-should-experimental-features-be-moved-out-from-experimental)
* [Can "experimental" features fail the CI temporarily?](#can-experimental-features-fail-the-ci-temporarily)

## What are "experimental" features?

"Experimental" features are features living in master branch, 
but Kata community thinks they're not ready for production use.
They are **always disabled** by default in Kata components releases,
and can only be enabled by users when they want to have a try.

We suggest you **NEVER** enable "experimental" features in production environment,
unless you know what breakage they can bring and have confidence to handle it by yourself.

Criteria of an experimental feature are:

* the feature breaks backward compatibility

compatibility is important to Kata Containers,
We will **NEVER** accept any new features/codes which break the backward compatibility of Kata components,
unless they are so important and there's no way to avoid the breakage.
If we decide to involve such changes, maintainers will help to make the decision that which Kata release
it should land.

Before it's landed as a formal feature, we allow the codes to be merged first into our master with the tag "experimental",
so it can improve in next few releases to be stable enough.

* the feature is not stable enough currently

Some features could be big, it adds/changes lots of codes so may need more tests.
Our CI can help guarantee correctness of the feature, but it may not cover all scenarios.
Before we're confident that the feature is ready for production use,
the feature can be marked as "experimental" first, and users can test it manually in their own environment if interested in it.

We make no guarantees about experimental features, they can be removed entirely at any point,
or become non-experimental at some release, so relative configuration options can change radically.

An experimental feature **MUST** have a descriptive name containing only lower-case characters, numbers or `_`, 
e.g. `new_hypervisor_2`, the name **MUST** be unique and will never be re-used in future.

## What's the difference between "WIP" and "experimental"?

"WIP"(work in progress) are usually used to mark the PR as incomplete before the PR can be reviewed and merged,
after the PR finishes its designed purpose(fix bugs, add new features etc) and all CI jobs pass, the codes can be merged into master branch.
After merging, we can still mark this part as "experimental", and leave some space for its evolution in future releases.

In one word, "experimental" can be unstable currently but it **MUST** be complete and functional, thus different from "WIP".

## When should "experimental" features be moved out from "experimental"?

That depends.

For the feature that breaks backward compatibility, we usually land it as formal feature in a major version bump(x in x.y.z, e.g. 2.0.0).
But for a new feature who becomes stable and ready, we can release it formally in any minor version bump.

Check Kata Container [versioning rules](https://github.com/kata-containers/documentation/blob/c556f1853f2e3df69d336de01ad4bb38e64ecc1b/Releases.md#versioning).

The experimental feature should state clearly in documentation the rationale for it to be experimental, 
and when it is expected to be non-experimental,
so that maintainers can consider to make it formal in right release.

## Can "experimental" features fail the CI temporarily?

No.

"Experimental" features are part of Kata master codes, it should pass all CI jobs or we can't merge them,
that's different from "WIP", a "WIP" PR can fail the CI temporarily before it can be reviewed and merged.

