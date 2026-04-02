# Security Policy

Kata Containers is a **rolling-release** project: every monthly release replaces the previous one, and only the _current_ release series receives security fixes. There are **no long-term-support branches**.

---

## Reporting a Vulnerability
### How to report

- **Keep it private first.**
  Please **do not** open a public GitHub issue or pull request for security problems.

- **Preferred: Use GitHub’s security advisory workflow.**
  Follow the official GitHub guide:
  [Creating a repository security advisory](https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/creating-a-repository-security-advisory#creating-a-security-advisory)

- **Alternative (if you don’t have a GitHub account):**
  Email security concerns to the Kata Containers security team at: **security@katacontainers.io**

### What happens after you submit

We follow the OpenSSF vulnerability-handling guidelines.
The table below shows the target timelines we hold ourselves to once we receive your report.
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

- **Early notification**: Contact the Kata Containers security team to be added to our downstream stakeholders list. We provide advance notification of embargoed security fixes to allow time for rebasing, testing, and release planning.
- **Embargo process**: Before public disclosure, we coordinate appropriate embargo periods to allow your team to incorporate the fix.
- **Mailing list**: Subscribe to the Kata Containers security notifications list (to be announced) for advance security patch notification.

---

## Disclosure Process & Fix Delivery

1. We develop the fix on a private branch.
2. Once validated, we coordinate embargo dates with downstream consumers to allow time for rebasing and testing.
3. We publish a GitHub security advisory, which automatically triggers CVE ID assignment.
4. The fix ships **most commonly** in the next regular monthly release (e.g., `v3.19`) when impact is moderate and waiting does not materially increase risk.
5. **Exception**: For Critical or High-severity issues, we may cut a point release (e.g., `v3.18.1`) to provide an update path for users still on the current series.
6. Upon public release, security details and CVE information are published in the GitHub release notes.

---

## Security Advisories & Release Notes

* Each patch or monthly release includes a **Security Bulletin** section in its GitHub *Release Notes* summarizing:
  * affected components & versions,
  * CVE identifiers (if assigned),
  * severity / CVSS score,
  * mitigation steps,
  * upgrade instructions.

* We do **not** publish separate “stable-branch” advisories because unsupported branches receive no fixes.

---

## Frequently Asked Questions

**Q: I run `v3.24` – will you patch it?**
A: No. Upgrade to the latest monthly release. Only the current monthly release receives security fixes.

**Q: I maintain a downstream distribution. Can I get early access to security patches?**
A: Yes. Contact the security team (see [SECURITY_CONTACTS](SECURITY_CONTACTS)) to be added to our downstream stakeholders list. We provide embargo periods to allow time for rebasing and testing before public disclosure.

**Q: What if I don't have a GitHub account? How do I report a vulnerability?**
A: Email security@katacontainers.io with a description of the issue. Include steps to reproduce if possible.

**Q: Where can I discuss a vulnerability once it is public?**
A: Open/continue a GitHub issue **after** the advisory is published, or use the `#kata-containers` Slack channel with a link to the official security advisory.

---

*Last updated:* 2026-04-02
