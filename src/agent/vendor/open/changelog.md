# Changelog

## 3.2.0 (2022-11-21)

### New Features

 - <csr-id-c3d2819d121ede284ba12d26ac3272c1f664c4ed/> upgrade `windows-sys` to more recent version.
   This mainly reduces build times for some, and may increase them for
   others, on windows only. If build times increase, try to upgrade
   `windows-sys` across the dependency tree.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - prepare changelog prior to release ([`20c6ee4`](https://github.com/Byron/open-rs/commit/20c6ee456d400416673d0e98058f55d50c78115a))
    - upgrade `windows-sys` to more recent version. ([`c3d2819`](https://github.com/Byron/open-rs/commit/c3d2819d121ede284ba12d26ac3272c1f664c4ed))
    - Upgrade to windows-sys v0.42 ([`4de95c7`](https://github.com/Byron/open-rs/commit/4de95c73503b19f810d7e669b73e261b1004e689))
    - Revert "Upgrade to windows-sys v0.42.0" ([`2aff3bd`](https://github.com/Byron/open-rs/commit/2aff3bd2a2e917377ef10dcc4104c6aaf5895bd4))
</details>

## 3.1.0 (2022-11-20)

**YANKED**

### New Features

 - <csr-id-a1c8dd79eb6c4f91a92aa631fd0d8bc163d1a05c/> upgrade `windows-sys` to more recent version.
   This mainly reduces build times for some, and may increase them for
   others, on windows only. If build times increase, try to upgrade
   `windows-sys` across the dependency tree.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 8 calendar days.
 - 65 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v3.1.0 ([`37bf011`](https://github.com/Byron/open-rs/commit/37bf011b68a5579254341de92e4d9b27ce71803b))
    - upgrade `windows-sys` to more recent version. ([`a1c8dd7`](https://github.com/Byron/open-rs/commit/a1c8dd79eb6c4f91a92aa631fd0d8bc163d1a05c))
    - Upgrade to windows-sys v0.42.0 ([`aba0a62`](https://github.com/Byron/open-rs/commit/aba0a628b1bf1be365fdbe2bc0200d0c98c7a2bf))
    - Update listed version number. ([`7a1cc83`](https://github.com/Byron/open-rs/commit/7a1cc838d5fe0218e8d1422b42a32023fd140e67))
    - Merge branch 'fmt' ([`f4dfeab`](https://github.com/Byron/open-rs/commit/f4dfeabf43b2ede234892e1204248a85313b51b5))
    - Point docs link to docs.rs rather than an outdated copy ([`52f96fc`](https://github.com/Byron/open-rs/commit/52f96fc20f9a9c0db3464b3f8f1a24f8045145f2))
    - Update Readme ([`98316c4`](https://github.com/Byron/open-rs/commit/98316c42a236018d51fdc3c65afa7338237fe964))
</details>

## 3.0.3 (2022-09-16)

### Bug Fixes

 - <csr-id-4c0fdb3bacd73c881c6e8178248c588932ec6196/> quote paths on windows to allow spaces in paths not be treated as multiple paths.
   Note that paths that are already quoted will also be quoted, as the
   current quoting implementation is unconditional.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release over the course of 60 calendar days.
 - 60 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v3.0.3 ([`9c69785`](https://github.com/Byron/open-rs/commit/9c697852432e5e34d5475706107e2f418b9296de))
    - quote paths on windows to allow spaces in paths not be treated as multiple paths. ([`4c0fdb3`](https://github.com/Byron/open-rs/commit/4c0fdb3bacd73c881c6e8178248c588932ec6196))
    - refactor ([`e0d5968`](https://github.com/Byron/open-rs/commit/e0d596880cd1d746d80927155092827614a7a3ef))
    - Fixed issue on Windows where a space in a path could cause problems with specific programs. ([`1ab9bc3`](https://github.com/Byron/open-rs/commit/1ab9bc37a0fc04d9fa033245d0c44392f2a2912a))
    - try to fix CI by not using nightly toolchains on windows ([`b20e01c`](https://github.com/Byron/open-rs/commit/b20e01cf590d82a05841af1c92428249fe21d838))
</details>

## 3.0.2 (2022-07-17)

### Bug Fixes

 - <csr-id-fe70aad1ee0c792b83e1c5faabda8d2c142cdabe/> Improve documentation about blocking behaviour.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 35 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#51](https://github.com/Byron/open-rs/issues/51)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#51](https://github.com/Byron/open-rs/issues/51)**
    - Improve documentation about blocking behaviour. ([`fe70aad`](https://github.com/Byron/open-rs/commit/fe70aad1ee0c792b83e1c5faabda8d2c142cdabe))
 * **Uncategorized**
    - Release open v3.0.2 ([`c7ea529`](https://github.com/Byron/open-rs/commit/c7ea5291ac6a26da7346f995fad5b3121b02f488))
</details>

## 3.0.1 (2022-06-12)

### Bug Fixes

 - <csr-id-df358d296fc40801e970654bf2b689577637db5e/> deprecate `that_in_background()` as `that()` is definitely non-blocking now.
   Note that we keep `with_in_background()` as it's unclear if a custom
   launcher blocks or not.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v3.0.1 ([`757f773`](https://github.com/Byron/open-rs/commit/757f773a6d7e3afa35c2cab6f3f4a44c7c8facee))
    - deprecate `that_in_background()` as `that()` is definitely non-blocking now. ([`df358d2`](https://github.com/Byron/open-rs/commit/df358d296fc40801e970654bf2b689577637db5e))
</details>

## 3.0.0 (2022-06-12)

A major release which simplifies the error type to resolve a significant problems that surfaced on
linux (and was present from day one).

### Bug Fixes (BREAKING)

 - <csr-id-0bdc6d64ed425b2627a7ba17614f44ba686536fb/> Assure `that(…)` is non-blocking on linux
   This change goes hand in hand with removing additional information
   from the error case which was the reason for the blocking issue
   on linux.
   
   Note that the top-level `Result` type was also removed.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 9 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v3.0.0 ([`3f51fb2`](https://github.com/Byron/open-rs/commit/3f51fb2e95a1f54c3ba54f349edefec34c25c7dc))
    - update changelog and docs ([`10b92f5`](https://github.com/Byron/open-rs/commit/10b92f55de77c508a6cbd95c344a3d923b9207c4))
    - refactor ([`475f002`](https://github.com/Byron/open-rs/commit/475f0021071fa1498a0fb5ca7d7336a3f4a35b7f))
    - Assure `that(…)` is non-blocking on linux ([`0bdc6d6`](https://github.com/Byron/open-rs/commit/0bdc6d64ed425b2627a7ba17614f44ba686536fb))
</details>

## 2.1.3 (2022-06-03)

A maintenance release which reduces compile times on windows by switching from `winapi` to the
`windows` crate.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 35 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v2.1.3 ([`bf6e99c`](https://github.com/Byron/open-rs/commit/bf6e99cb578cd3c90eed3ff4fddd712b26982e21))
    - prepare new release ([`c1844c7`](https://github.com/Byron/open-rs/commit/c1844c7557b5e2d3c96cc19f4bc7e3fa7f2ef7d3))
    - Merge branch 'windows-sys' ([`246ddc8`](https://github.com/Byron/open-rs/commit/246ddc837d19760e9ad255ce31fbb6dfdac71738))
    - Switch to windows-sys ([`a95a288`](https://github.com/Byron/open-rs/commit/a95a2881064ec1a348031b2050d2873df2def31e))
</details>

## 2.1.2 (2022-04-29)

<csr-id-85f4dfdafe6119af5b3a5d8f079279818d3d61ee/>

### Other

 - <csr-id-85f4dfdafe6119af5b3a5d8f079279818d3d61ee/> add Heiku platform support

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 54 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v2.1.2 ([`ee25446`](https://github.com/Byron/open-rs/commit/ee25446e872c18253bfe4c974b534ea8dd993cc2))
    - update changelog ([`45e0388`](https://github.com/Byron/open-rs/commit/45e0388e3c0a1b255b5868d6e0c3a540b75c33e9))
    - add platform support ([`85f4dfd`](https://github.com/Byron/open-rs/commit/85f4dfdafe6119af5b3a5d8f079279818d3d61ee))
</details>

## 2.1.1 (2022-03-05)

A maintenance release which allows boxed values in parameter position.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 11 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v2.1.1 ([`18f730d`](https://github.com/Byron/open-rs/commit/18f730d7d40f7e27002479adb41b342413235ce7))
    - prepare changelog ([`d569761`](https://github.com/Byron/open-rs/commit/d569761a7c6c57f92e48fc6ac195baf13df8666d))
    - Revert rust edition version ([`9441d6c`](https://github.com/Byron/open-rs/commit/9441d6c87419f94e0ebaffdf69f9b01f0aec4ddb))
    - Update to 2021 edition and remove Sized bound ([`2601e4e`](https://github.com/Byron/open-rs/commit/2601e4eff11a77a7ccd5acfa3215eb76450fe18c))
</details>

## 2.1.0 (2022-02-21)

* add support for illumnos

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 8 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v2.1.0 ([`a49e9cc`](https://github.com/Byron/open-rs/commit/a49e9ccac9ea89dabc19b1a0215378ede887260b))
    - Update changelog ([`b56050f`](https://github.com/Byron/open-rs/commit/b56050f41fc04a2d5ec61f20451df534315f7d74))
    - add Illumos support ([`5d43c13`](https://github.com/Byron/open-rs/commit/5d43c13e5418f1d34b44cab71ee7306402fe5823))
</details>

## 2.0.3 (2022-02-13)

On MacOS, specify the `open` program explicitly by path, instead of relying on a similarly named program to be available
in the `PATH`.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 74 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release open v2.0.3 ([`3b5e74d`](https://github.com/Byron/open-rs/commit/3b5e74dbab169ee2a22c9de0b3a5923dc7e6937e))
    - Prepare changelog ([`1c7e10f`](https://github.com/Byron/open-rs/commit/1c7e10f94c30598bdc2e4ae482d38b2f46928ebf))
    - use full path for `open` command on macOS ([`8f7c92a`](https://github.com/Byron/open-rs/commit/8f7c92ab1adf936cd43e4ba0eb1934e2c73763f7))
</details>

## 2.0.2 (2021-11-30)

### Bug Fixes

 - <csr-id-30a144ac15acffbc78005cd67d3f783aa2526498/> Prevent deadlocks due to filled pipe on stderr

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 106 calendar days.
 - 128 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 1 unique issue was worked on: [#85](https://github.com/Byron/open-rs/issues/85)

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **[#85](https://github.com/Byron/open-rs/issues/85)**
    - Prevent deadlocks due to filled pipe on stderr ([`30a144a`](https://github.com/Byron/open-rs/commit/30a144ac15acffbc78005cd67d3f783aa2526498))
 * **Uncategorized**
    - Release open v2.0.2 ([`1d94593`](https://github.com/Byron/open-rs/commit/1d94593fa7be75ffdafcb7614c0f68fe4485f07a))
    - update changelog ([`e9a2f05`](https://github.com/Byron/open-rs/commit/e9a2f05ec8248b3723779dfead6fbd4827a2f929))
    - Release open v2.0.1 ([`066a591`](https://github.com/Byron/open-rs/commit/066a591823ddebb2904959b6395bc945c22ba213))
    - Merge pull request #36 from apogeeoak/documentation ([`fc755d3`](https://github.com/Byron/open-rs/commit/fc755d343cede927c06e1735e8d14ed3858d2582))
    - Add no_run to documentation examples. ([`7c97658`](https://github.com/Byron/open-rs/commit/7c9765891b86d5d6168556e8f5363641f57e130d))
    - Update documentation. ([`5dd987f`](https://github.com/Byron/open-rs/commit/5dd987f3d25ebf3c82394d1225b836aefaf93b5d))
</details>

## v2.0.1 (2021-08-15)

Update documentation. No functionality changes.

## v2.0.0 (2021-07-25)

**Breaking**: Change result from `io::Result<ExitStatus>` to `io::Result<()>`.
Commands that exit with a successful exit status result in `Ok`, otherwise an `Err` variant is created.
Previously it was easy to receive an `Ok(ExitStatus)` but forget to actually check the status. Along with
issues with particular programs reporting success even on error, doing error handling correctly was
close to impossible.
This releases alleviates most of the issues.

## Notes

`wslview` always reports a 0 exit status, even if the path does not exist, which results in false positives.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release.
 - 8 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - (cargo-release) version 2.0.0 ([`bc2e36f`](https://github.com/Byron/open-rs/commit/bc2e36f5d61b81974420cd62650d743afd4b6824))
    - Update changelog ([`6659519`](https://github.com/Byron/open-rs/commit/665951968a2d99cbebaf41bc2dd564ea9d6dc93c))
    - Merge branch 'result_type' ([`0226df6`](https://github.com/Byron/open-rs/commit/0226df6be4abd85f0c8f8001532d0c67ad231a49))
    - Merge pull request #34 from apogeeoak/rustfmt ([`05f02be`](https://github.com/Byron/open-rs/commit/05f02be302377d669350f30991c2f80e6a729bc7))
    - Encode unsuccessful exit status in Err. ([`668734e`](https://github.com/Byron/open-rs/commit/668734ee8d4a3b3c48c9d3ad892280ce8e71f943))
    - Add empty rustfmt.toml file to enforce defaults. ([`1faabe3`](https://github.com/Byron/open-rs/commit/1faabe36fcaa4986b25bbc91a08d41759d1b8b88))
</details>

## v1.7.1 (2021-07-17)

* Improved support for [Windows Subsystem for Linux](https://github.com/Byron/open-rs/pull/33#issue-691044025)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 9 commits contributed to the release.
 - 89 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - (cargo-release) version 1.7.1 ([`c5c0bf7`](https://github.com/Byron/open-rs/commit/c5c0bf7ca070fb81fbbb1bd06d51b490a8f8bf1a))
    - prepare release ([`866740b`](https://github.com/Byron/open-rs/commit/866740b10e1f5b03fc4a3aab847546b7c378b6d9))
    - Be bold and assert ([`1dfb789`](https://github.com/Byron/open-rs/commit/1dfb7892554087ab07c7c0da8ef863d368e109e3))
    - cargo fmt ([`5bc5e8e`](https://github.com/Byron/open-rs/commit/5bc5e8e739915d4850d4a973d9cf13591aa337cc))
    - Improve support for wsl. ([`428ff97`](https://github.com/Byron/open-rs/commit/428ff979979760132d7c583df6834c3349132350))
    - Merge pull request #32 from apogeeoak/exit_status ([`81d8c40`](https://github.com/Byron/open-rs/commit/81d8c406cdf9405e31965a5aea9a5d21da812433))
    - cargo fmt ([`215227a`](https://github.com/Byron/open-rs/commit/215227a3385aa2624d32567eebb08af49e258b60))
    - clarify what the error handler does ([`4f87a78`](https://github.com/Byron/open-rs/commit/4f87a7888049b182ede9e00a057c2cc625152ef9))
    - Handle unsuccessful exit status. ([`d2d35af`](https://github.com/Byron/open-rs/commit/d2d35af2f582249030fc569854450ac12e3c08d4))
</details>

## v1.7.0 (2021-04-18)

* Add `gio` support on unix platforms

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 38 calendar days.
 - 38 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - (cargo-release) version 1.7.0 ([`ac09da1`](https://github.com/Byron/open-rs/commit/ac09da180256c5974427313b845b928199faf913))
    - update changelog ([`e39f357`](https://github.com/Byron/open-rs/commit/e39f357627288d971b6436f873ac2949fa534548))
    - Re-enable CI after branch renaming ([`0db1b1a`](https://github.com/Byron/open-rs/commit/0db1b1ad11853750b8c22a701438d0e3e149821b))
    - Merge pull request #31 from City-busz/patch-1 ([`10fd4a7`](https://github.com/Byron/open-rs/commit/10fd4a7183c9137bb1afee5a9a6d3dcc87eb821a))
    - Remove unnecessary allocation ([`6a1766a`](https://github.com/Byron/open-rs/commit/6a1766a602fa3354827b06d7b5dbf8f694b86690))
    - Add support for gio open on Linux ([`90bc634`](https://github.com/Byron/open-rs/commit/90bc6348e00e2e42cc0f7ed3eb7746d6e749749e))
    - Update changelog to reflect 1.5.1 is also yanked ([`ccbae5d`](https://github.com/Byron/open-rs/commit/ccbae5d122cb0b8cff58d9125ced2d0211e82ec9))
</details>

## v1.6.0 (2021-03-10)

* Add IOS support
* Restore Android support

## v1.5.1 (2021-03-03) - YANKED

YANKED as it would erroneously exclude Android from the list of supported platforms, making it a breaking release for some despite
the minor version change.

* Use shell instead of explorer on windows, reverting the original behaviour.

## v1.5.0 (2021-02-28) - YANKED

YANKED to avoid potential for breakage by using 'explorer.exe' to open URLs.

* Use 'explorer' on Windows instead of a shell.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release over the course of 7 calendar days.
 - 7 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - (cargo-release) version 1.6.0 ([`68613a5`](https://github.com/Byron/open-rs/commit/68613a5cabc1e650ab36ce5c2802c1b29e6af569))
    - more coherent ordering of target_os attributes ([`c058966`](https://github.com/Byron/open-rs/commit/c058966e5ec4cbd52c7cb50e5ee29afdac08cc15))
    - Restore android support ([`9e20f22`](https://github.com/Byron/open-rs/commit/9e20f22453955e5d7adba94cea8321961fac30ed))
    - adjust changelog in preparation for release ([`9bfefd0`](https://github.com/Byron/open-rs/commit/9bfefd0e38ccce6f898ac895b10ab5555606744f))
    - Merge pull request #28 from aspenluxxxy/ios ([`049f698`](https://github.com/Byron/open-rs/commit/049f698714cacfad9142db492d9f309af567d26a))
    - Bring back Android support ([`ef91705`](https://github.com/Byron/open-rs/commit/ef9170527d6e9eb58e2b11e73e2756ccbc6b170b))
    - Add iOS support ([`00119a7`](https://github.com/Byron/open-rs/commit/00119a7e5b00889828ab9d38dd5959a519f22b1d))
    - run cargo-fmt ([`330c2d0`](https://github.com/Byron/open-rs/commit/330c2d02f92e3660a86158713a9a9c3aba542094))
</details>

## v1.5.1 (2021-03-03)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release.
 - 3 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - (cargo-release) version 1.5.1 ([`147f428`](https://github.com/Byron/open-rs/commit/147f428762c84d0353644b5b846756ac38b29670))
    - changelog for patch ([`9400b1a`](https://github.com/Byron/open-rs/commit/9400b1a67ccf02ff757ecb392d179440ddb98eac))
    - minor refactor ([`67ea295`](https://github.com/Byron/open-rs/commit/67ea2950aa2c478c8cd63764145ad53ad55bdd11))
    - Merge pull request #27 from hybras/master ([`b58fa52`](https://github.com/Byron/open-rs/commit/b58fa52eb8ee46a789c864b7132e8375fe7efa77))
    - Keep Fork up to date with upstream ([`f113b80`](https://github.com/Byron/open-rs/commit/f113b80374ed1412d2d86e79b79f7ac9ef39a2fc))
    - Revert "Add missing Command import" ([`7ff85da`](https://github.com/Byron/open-rs/commit/7ff85da679de7cd17155c4ea27d0f89fda6dff0a))
    - Revert "Use the file explorer to open windows url's" ([`b2a79f6`](https://github.com/Byron/open-rs/commit/b2a79f6b93feef3a59ce57d865334d757e642540))
</details>

## v1.5.0 (2021-02-28)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 18 commits contributed to the release over the course of 321 calendar days.
 - 356 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - minor version bump ([`831d440`](https://github.com/Byron/open-rs/commit/831d4404ee3ab9d76a416f69ee586f7d037f4840))
    - Merge branch 'hybras/master' ([`e8bb206`](https://github.com/Byron/open-rs/commit/e8bb20654ac8b8ffbdfebf70f9aa1f0d3cdc0a33))
    - Delete completed TODO file ([`6c6bad0`](https://github.com/Byron/open-rs/commit/6c6bad075a5dcdc12670ec885000e26810bcf4fc))
    - Remove unneeded pub ([`3507b55`](https://github.com/Byron/open-rs/commit/3507b55dcaa30db5673cbe8b7a405db4f00245ac))
    - Remove user specific dir from gitgnore ([`021bb15`](https://github.com/Byron/open-rs/commit/021bb150d6066b111bdb04d2c4340dc9172db562))
    - Add missing Command import ([`c910278`](https://github.com/Byron/open-rs/commit/c9102785d58cc955595eb189bd89a2ff82a539f0))
    - Use the file explorer to open windows url's ([`4545425`](https://github.com/Byron/open-rs/commit/45454254b6e07fd88e398e8de86b55863f369373))
    - Mark completed todo items ([`db518e9`](https://github.com/Byron/open-rs/commit/db518e9063933df824c4bb0e0c560bc73ef1b700))
    - Use which in non-macOS unix ([`ef8ab99`](https://github.com/Byron/open-rs/commit/ef8ab99d65ce7baf03d43304b3c0cb48e816e411))
    - Change cfg(not(any(bad)) to cfg(any(good)) ([`204f0ca`](https://github.com/Byron/open-rs/commit/204f0ca89f522ca4e6dc31b0cdefc3bcd434909b))
    - Modularize Code ([`cb5bbd3`](https://github.com/Byron/open-rs/commit/cb5bbd3287bf2ca66e6ea3afefb149e4fe12bdd8))
    - Add todo's ([`311ad44`](https://github.com/Byron/open-rs/commit/311ad44c50ddba910c13f3cd85326522accc8e23))
    - optimize manifest includes ([`c3d8262`](https://github.com/Byron/open-rs/commit/c3d826220e59040d6d08d707ac771ba817165a07))
    - See if we can run cargo clippy and rustfmt as well ([`c90687d`](https://github.com/Byron/open-rs/commit/c90687de90eb3731ec508c8d3df639de582fb163))
    - Actually link to the correct workflow when clicking the badge ([`6765b42`](https://github.com/Byron/open-rs/commit/6765b424010b55e23568924786700a3795e694dc))
    - bye bye travis, we had a great time ([`aa28a85`](https://github.com/Byron/open-rs/commit/aa28a858dfe8be9c34e3fd6a6df67722baec4df1))
    - rename workflow in file as well ([`6bfc6d2`](https://github.com/Byron/open-rs/commit/6bfc6d2e9efdbd656a37531fe43cca6ab443a2b9))
    - try cross-platform testing based on cross-platform binary builds ([`d62e50d`](https://github.com/Byron/open-rs/commit/d62e50d7b1944597468b2c983047e236ae9ff08f))
</details>

## v1.4.0 (2020-03-08)

* add `open::with(path, app)` and `open::with_in_background(…)`

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 25 calendar days.
 - 25 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Adjust doc; cut release ([`ead2494`](https://github.com/Byron/open-rs/commit/ead2494b783ffd0e393972aeb1f82adaf39fe0d3))
    - Cargo fmt ([`94b129a`](https://github.com/Byron/open-rs/commit/94b129ad998729967a856f19f74e4628957ea99b))
    - fixed import bug ([`e98ec3d`](https://github.com/Byron/open-rs/commit/e98ec3d79ef199dc16f3ce65b766aa0110abaaf0))
    - update README.md ([`9efaee0`](https://github.com/Byron/open-rs/commit/9efaee0b5402c725e2c152643d448182881a2898))
    - add with function ([`9b83669`](https://github.com/Byron/open-rs/commit/9b83669e8c463648b6f4149e84fcb1e00d68f49b))
    - (cargo-release) start next development iteration 1.3.5-alpha.0 ([`d3db8c7`](https://github.com/Byron/open-rs/commit/d3db8c748be2e65865aed7246cd8eaeaacd4ef8a))
</details>

## v1.3.4 (2020-02-11)

<csr-id-5c1497c6d09a829d4be19e9bd3eec5557efce370/>

* Add LICENSE.md and README.md into the crates.io tarball.

### Chore

 - <csr-id-5c1497c6d09a829d4be19e9bd3eec5557efce370/> Include README/LICENSE into a release tarball

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 9 calendar days.
 - 184 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - bump version ([`547fd28`](https://github.com/Byron/open-rs/commit/547fd283e684470e5e981a658d98c31208da1e8b))
    - Include README/LICENSE into a release tarball ([`5c1497c`](https://github.com/Byron/open-rs/commit/5c1497c6d09a829d4be19e9bd3eec5557efce370))
    - Further simplification ([`9f285e5`](https://github.com/Byron/open-rs/commit/9f285e559878f3da2eb54f50aa88632385618f7c))
    - Update to edition 2018 ([`dfca673`](https://github.com/Byron/open-rs/commit/dfca6736f69555e3285786bb10719adb0ae1d0c7))
</details>

## v1.3.3 (2020-02-01)

* update code and crate to Edition 2018

## v1.3.2 (2019-08-11)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 13 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Bump version ([`9d9e40c`](https://github.com/Byron/open-rs/commit/9d9e40cc9b68266652a5ac21915b558b812ee444))
    - Improve documentation ([`d45e4c6`](https://github.com/Byron/open-rs/commit/d45e4c6103f95027b3ef397d5f03a8b75bcdb03d))
    - Add that_in_background ([`5927784`](https://github.com/Byron/open-rs/commit/5927784721174259af5e6f3d07f724f5b6e89501))
</details>

## v1.3.1 (2019-07-28)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 25 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - New minor release with wslview support ([`cb41dce`](https://github.com/Byron/open-rs/commit/cb41dce487b9936c3bf93d242dc6ff70a7924f0a))
    - Use wslview on WSL, try it as last binary ([`0babfd0`](https://github.com/Byron/open-rs/commit/0babfd0abfa266739a8aaadf8fc936c8c061ac0b))
    - Add support for Linux in WSL through wslu/wslview ([`0a43537`](https://github.com/Byron/open-rs/commit/0a4353764a17579e92ae97ea08ea226ace5cc86a))
</details>

## v1.2.3 (2019-07-03)

<csr-id-c2908176e2bb982a679d7097584e584a53deaf15/>

### Chore

 - <csr-id-c2908176e2bb982a679d7097584e584a53deaf15/> Exclude unneeded files from crates.io

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 16 calendar days.
 - 331 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Bump version ([`2e8d245`](https://github.com/Byron/open-rs/commit/2e8d2453d801cb27b311b27bf49b06791a35958a))
    - Supress stdout and stderr for non-windows platforms ([`4e3574a`](https://github.com/Byron/open-rs/commit/4e3574a20a84c8a0d681e11ec351d20e35b73ea4))
    - Exclude unneeded files from crates.io ([`c290817`](https://github.com/Byron/open-rs/commit/c2908176e2bb982a679d7097584e584a53deaf15))
</details>

## v1.2.2 (2018-08-05)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 6 commits contributed to the release over the course of 234 calendar days.
 - 314 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - bump version: better way to open things on windows ([`78e94aa`](https://github.com/Byron/open-rs/commit/78e94aa545e531f99aabcd0a328adc44e4ed06a6))
    - Use ShellExecute rather than start.exe on windows ([`e2fc4b1`](https://github.com/Byron/open-rs/commit/e2fc4b1061ef105e237b4dda1ffa03eaf3a1cdb4))
    - Small optimizations and stylistic improvements ([`88ddb6f`](https://github.com/Byron/open-rs/commit/88ddb6febe811fa8f5ab12b32cbf2a716676fb53))
    - Adjust code style ([`dd9dde6`](https://github.com/Byron/open-rs/commit/dd9dde6aa4cd47ca57378ac018a66dbbcd661d44))
    - Add crates version badge ([`4e41d8b`](https://github.com/Byron/open-rs/commit/4e41d8bdf0c3bbca84efc1de9759e06839208c86))
    - Run latest rustfmt ([`ec5c7ab`](https://github.com/Byron/open-rs/commit/ec5c7ab817f3978212b0230512b75a1a8b5374f1))
</details>

## v1.2.1 (2017-09-24)

<csr-id-79bc73b7ca0927f0594670bcc23de989693275c0/>

### Other

 - <csr-id-79bc73b7ca0927f0594670bcc23de989693275c0/> improve example

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 7 commits contributed to the release over the course of 178 calendar days.
 - 236 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Version bump - fix windows 'start' invocation ([`d52cfb2`](https://github.com/Byron/open-rs/commit/d52cfb252e3feee3d3793bc91d28e9cf6f193947))
    - Merge pull request #11 from reiner-dolp/master ([`59dd0bd`](https://github.com/Byron/open-rs/commit/59dd0bd64921594ba3ed59ddb373aac55d33c95d))
    - Merge pull request #9 from skade/patch-1 ([`c017217`](https://github.com/Byron/open-rs/commit/c017217f4b975a297d2116e35fb230b3b370c9cf))
    - fix filenames with spaces on windows ([`a631235`](https://github.com/Byron/open-rs/commit/a631235c285b5f48ce63a52cbf7d70f51439db06))
    - Fix a small typo ([`89caa59`](https://github.com/Byron/open-rs/commit/89caa594bf2e16929dc74565e197ba2d3cbd8390))
    - Merge pull request #7 from tshepang/misc ([`0ccdbd0`](https://github.com/Byron/open-rs/commit/0ccdbd08f450f364ce3538fe28a05f41c8188ae6))
    - improve example ([`79bc73b`](https://github.com/Byron/open-rs/commit/79bc73b7ca0927f0594670bcc23de989693275c0))
</details>

## v1.2.0 (2017-01-31)

<csr-id-37a253c89b1241b6f6ca0d3cafc8baa936aa274f/>

* **windows**: escape '&' in URLs. On windows, a shell is used to execute the command, which
  requires certain precautions for the URL to open to get through the interpreter.

### Chore

 - <csr-id-37a253c89b1241b6f6ca0d3cafc8baa936aa274f/> v1.2.0

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 4 calendar days.
 - 295 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - v1.2.0 ([`37a253c`](https://github.com/Byron/open-rs/commit/37a253c89b1241b6f6ca0d3cafc8baa936aa274f))
    - Merge pull request #6 from DenisKolodin/win-escape-fix ([`d0b3b35`](https://github.com/Byron/open-rs/commit/d0b3b35b4881da297cc44847dc5f3500c25eef61))
    - Escape GET parameters for Windows ([`3f4319c`](https://github.com/Byron/open-rs/commit/3f4319c79e293fb8141e6574db710a7f97e0f1d4))
</details>

## v1.1.1 (2016-04-10)

<csr-id-da45d9bad33fd9ed9659ec56ffe3b31d310253ca/>

### Bug Fixes

* **cargo:**  no docs for open ([31605e0e](https://github.com/Byron/open-rs/commit/31605e0eddfb0cf8db635dd4d86131bc46beae78))
 - <csr-id-31605e0eddfb0cf8db635dd4d86131bc46beae78/> no docs for open
   And I thought I did that, but disabled tests only ... .

### Improvements

* **api:**  allow OSStrings instead of &str ([1d13a671](https://github.com/Byron/open-rs/commit/1d13a671f2c9bd9616bf185fac77b32da1dcf8ee))

### Other

 - <csr-id-da45d9bad33fd9ed9659ec56ffe3b31d310253ca/> allow OSStrings instead of &str
   Actually I can only hope that ordinary &str will still be fine.
   Technically, I think they should ... but we shall see.

## 25c0e398 (2015-07-08)

### Features

* **open**  added 'open' program ([a4c3a352](https://github.com/Byron/open-rs/commit/a4c3a352c8f912211d5ab48daaf41cb847ebcc0c))

### Bug Fixes

* **cargo**  description added ([0fcafb56](https://github.com/Byron/open-rs/commit/0fcafb56cdb5d154b3e983d17c93a1dd7c665426))
* **open**
  * use result ([25c0e398](https://github.com/Byron/open-rs/commit/25c0e398856c24a2daf0444640567ed3fd2f4307))
  * don't use 'open' on linux ([30c96b1c](https://github.com/Byron/open-rs/commit/30c96b1cb95c1e03bede218b8fb03bbd9ada9317))
  * linux uses open before anything else ([4696d1a5](https://github.com/Byron/open-rs/commit/4696d1a5ec80691e97bb1be4261d4f79ee0ade4d))
* don't use 'open' on linux ([30c96b1c](https://github.com/Byron/open-rs/commit/30c96b1cb95c1e03bede218b8fb03bbd9ada9317))
* linux uses open before anything else ([4696d1a5](https://github.com/Byron/open-rs/commit/4696d1a5ec80691e97bb1be4261d4f79ee0ade4d))
 - <csr-id-31605e0eddfb0cf8db635dd4d86131bc46beae78/> no docs for open
   And I thought I did that, but disabled tests only ... .

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 276 calendar days.
 - 276 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - allow OSStrings instead of &str ([`da45d9b`](https://github.com/Byron/open-rs/commit/da45d9bad33fd9ed9659ec56ffe3b31d310253ca))
    - Merge pull request #2 from hoodie/master ([`ff32bea`](https://github.com/Byron/open-rs/commit/ff32beac235d5702a14752a9166ce3a168c6b250))
    - taking T:AsRef<OsStr> instead of &str ([`2540a0a`](https://github.com/Byron/open-rs/commit/2540a0a6abc4b27d6553400e7aef62e3ef94020d))
    - no docs for open ([`31605e0`](https://github.com/Byron/open-rs/commit/31605e0eddfb0cf8db635dd4d86131bc46beae78))
</details>

<csr-unknown>
don’t use ‘open’ on linux (https://github.com/Byron/open-rs/commit/30c96b1cb95c1e03bede218b8fb03bbd9ada931730c96b1c)linux uses open before anything else (https://github.com/Byron/open-rs/commit/4696d1a5ec80691e97bb1be4261d4f79ee0ade4d4696d1a5)<csr-unknown/>

## v1.1.0 (2015-07-08)

<csr-id-a5557d5c096983cf70f59b1807cb6fbe2b6dab5e/>
<csr-id-8db67f5874b007ea3710ed9670e88ad3f49b6d7d/>
<csr-id-d816380f9680a9d56e22a79e025dc6c2073fb439/>
<csr-id-bf8c9a11f4c1b1ac17d684a31c90d2a38255045e/>
<csr-id-210ec6ef37ba7d230a0cc367e979173a555fa092/>

### Chore

 - <csr-id-a5557d5c096983cf70f59b1807cb6fbe2b6dab5e/> v1.1.0
   * added clog configuration and changelog
 - <csr-id-8db67f5874b007ea3710ed9670e88ad3f49b6d7d/> use stable instead of beta
 - <csr-id-d816380f9680a9d56e22a79e025dc6c2073fb439/> switch to travis-cargo
 - <csr-id-bf8c9a11f4c1b1ac17d684a31c90d2a38255045e/> added sublime-rustc-linter cfg
   [skip ci]

### Other

 - <csr-id-210ec6ef37ba7d230a0cc367e979173a555fa092/> start is a cmd command, not an executable

### Documentation

 - <csr-id-c2e31d55da439e30639da2d014951e2eb2b851ff/> added travis badge
   [skip ci]

### New Features

 - <csr-id-a4c3a352c8f912211d5ab48daaf41cb847ebcc0c/> added 'open' program
   Which uses the `open` library to open any path or url.

### Bug Fixes

<csr-id-30c96b1cb95c1e03bede218b8fb03bbd9ada9317/>
<csr-id-4696d1a5ec80691e97bb1be4261d4f79ee0ade4d/>
<csr-id-0fcafb56cdb5d154b3e983d17c93a1dd7c665426/>

 - <csr-id-25c0e398856c24a2daf0444640567ed3fd2f4307/> use result
   I wonder why that was not shown when I compiled it
 - <csr-id-8b4e1558f09937c555ab381ea6399a2c0758c23d/> (07560d233 2015-04-20) (built 2015-04-19)
   * use std::io consistently
* adjust to improved `Command` API

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 13 commits contributed to the release over the course of 130 calendar days.
 - 130 days passed between releases.
 - 12 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - v1.1.0 ([`a5557d5`](https://github.com/Byron/open-rs/commit/a5557d5c096983cf70f59b1807cb6fbe2b6dab5e))
    - use result ([`25c0e39`](https://github.com/Byron/open-rs/commit/25c0e398856c24a2daf0444640567ed3fd2f4307))
    - added 'open' program ([`a4c3a35`](https://github.com/Byron/open-rs/commit/a4c3a352c8f912211d5ab48daaf41cb847ebcc0c))
    - Merge pull request #1 from oli-obk/patch-1 ([`dee0000`](https://github.com/Byron/open-rs/commit/dee00005fa1089e97fc4e193c934f6e7b3104333))
    - start is a cmd command, not an executable ([`210ec6e`](https://github.com/Byron/open-rs/commit/210ec6ef37ba7d230a0cc367e979173a555fa092))
    - use stable instead of beta ([`8db67f5`](https://github.com/Byron/open-rs/commit/8db67f5874b007ea3710ed9670e88ad3f49b6d7d))
    - switch to travis-cargo ([`d816380`](https://github.com/Byron/open-rs/commit/d816380f9680a9d56e22a79e025dc6c2073fb439))
    - added sublime-rustc-linter cfg ([`bf8c9a1`](https://github.com/Byron/open-rs/commit/bf8c9a11f4c1b1ac17d684a31c90d2a38255045e))
    - (07560d233 2015-04-20) (built 2015-04-19) ([`8b4e155`](https://github.com/Byron/open-rs/commit/8b4e1558f09937c555ab381ea6399a2c0758c23d))
    - don't use 'open' on linux ([`30c96b1`](https://github.com/Byron/open-rs/commit/30c96b1cb95c1e03bede218b8fb03bbd9ada9317))
    - linux uses open before anything else ([`4696d1a`](https://github.com/Byron/open-rs/commit/4696d1a5ec80691e97bb1be4261d4f79ee0ade4d))
    - description added ([`0fcafb5`](https://github.com/Byron/open-rs/commit/0fcafb56cdb5d154b3e983d17c93a1dd7c665426))
    - added travis badge ([`c2e31d5`](https://github.com/Byron/open-rs/commit/c2e31d55da439e30639da2d014951e2eb2b851ff))
</details>

## v1.0.0 (2015-02-27)

### New Features

 - <csr-id-6fbf79011577d465d9fed94a07a5f75b63199609/> from zero to 1.0.0
   Contains everything, including
   
   * API docs
* usage
* CI

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 1 commit contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - from zero to 1.0.0 ([`6fbf790`](https://github.com/Byron/open-rs/commit/6fbf79011577d465d9fed94a07a5f75b63199609))
</details>

