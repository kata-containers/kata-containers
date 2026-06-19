# Security Policy

Kata Containers is a **rolling-release** project: every monthly release replaces the previous one, and only the _current_ release series receives security fixes. There are **no long-term-support branches**.

---

## Reporting a Vulnerability

### How to report

- **Keep it private first.**
  Please **do not** open a public GitHub issue or pull request for security problems.

- **Use GitHub’s security advisory workflow**
  Follow the official GitHub guide:
  [Report a vulnerability privately](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately)

### What happens after you submit

We follow the OpenSSF vulnerability-handling guidelines.
The table below shows the target timelines we aim for once we receive your report.
These are independent objectives, not sequential steps.

| Objective | Target time | Notes |
|-----------|-------------|-------|
| **Initial acknowledgement** | ≤ 14 calendar days | Maintainers confirm receipt and start triage. |
| **Triage & CVSS-v3.1 scoring** | ≤ 30 days | We assign severity and plan remediation. |
| **Fix availability** | Next scheduled monthly release<br />(or an out-of-band patch for Critical/High issues) | We may cut a `vX.Y.Z` patch if waiting a month poses undue risk. |
| **CVE assignment** | Before public disclosure | GitHub automatically requests a CVE ID when we publish the security advisory. |

---

## Supported Versions

| Release | First published | Security-fix window |
|---------|-----------------|---------------------|
| **Latest monthly release** | see `git tag --sort=-creatordate \| head -1` | Actively maintained |
| Any prior release | — | **Unsupported** – please upgrade |

> **Why no backports?**
> Kata’s architecture evolves quickly; back-porting patches would re-introduce the very maintenance burden we avoid by using a rolling model.

### For Downstream Distributions & Vendors

If you maintain a downstream distribution or integration of Kata Containers:

- **Embargo process**: Please be aware that security vulnerabilities are fixed in private as part of an embargo period. During this period only the Kata Containers Vulnerability Management Team (VMT) and very limited number of trusted contributors have access to the vulnerability report and participate in fixing the issue.
- **Early notification**: The Kata Containers VMT sends embargo notifications to the private embargo-notice mailing list a few days in advance of public announcements, to allow downstream stakeholders time to test the fix and prepare to move to the new release that contains the public fix.

---

## Security Advisories & Release Notes

- Where applicable, each release includes a **Security** section in its GitHub _Release Notes_ which contains the list of CVEs we have addressed in that release:

- We do **not** publish separate “stable-branch” advisories because unsupported branches receive no fixes.

- To see the list of published security advisories please visit the [security tab](https://github.com/kata-containers/kata-containers/security) in the kata-containers repository.

---

## Frequently Asked Questions

#### Q: I run `v3.24` – will you patch it?

A: No. Upgrade to the latest monthly release. Only the current monthly release receives security fixes.

#### Q: Where can I discuss a vulnerability once it is public?

A: Open/continue a GitHub issue **after** the advisory is published, or use the `#general` channel in the [Kata Containers Slack workspace](https://join.slack.com/t/katacontainers/shared_invite/zt-16w1u6usn-sK871qbMxVN8KsCP5Gr56A) with a link to the official security advisory.

---

_Last updated:_ 2026-06-12
