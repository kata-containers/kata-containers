# Security Policy

Kata Containers is a **rolling-release** project: every monthly release replaces the previous one, and only the _current_ release series receives security fixes. There are **no long-term-support branches**.

---

## Reporting a Vulnerability

1. **Private first.**  
   Do **not** open a public GitHub issue or pull request.

2. **Use the offical Github documentation on how to report a vulnerability.**
   [Creating a repository security advisory](https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/creating-a-repository-security-advisory#creating-a-security-advisory)

3. **Response targets (OpenSSF guidelines).**  
   | Action | Target time | Notes |
   | ------ | ----------- | ----- |
   | Initial maintainer response | **≤ 14 calendar days** | Acknowledge receipt and begin triage. |
   | Triage & severity scoring | **≤ 30 days** | We follow CVSS v3.1. |
   | Fix availability | **Next scheduled monthly release**<br/>(or an out-of-band patch release for Critical/High issues) | We may cut `vX.Y.Z` if waiting a full month poses undue risk. |

---

## Supported Versions

| Release | First published | Security-fix window |
|---------|-----------------|---------------------|
| **Latest monthly release** | see `git tag --sort=-creatordate \| head -n 1` | Actively maintained |
| Any prior release | — | **Unsupported** – please upgrade |

> **Why no backports?**  
> Kata’s architecture evolves quickly; back-porting patches would re-introduce the very maintenance burden we avoid by using a rolling model.

---

## Disclosure Process & Fix Delivery

1. We develop the fix on a private branch.  
2. Once validated, we coordinate embargo dates with downstream consumers when appropriate.  
3. The fix ships in **either**:
   * Common: The next regular monthly release (e.g., `v3.19`) when impact is moderate and waiting does not materially increase risk, **or**
   * Exception: A point release (e.g., `v3.18.1`) if the vulnerability affects only the current series.
4. After the fix is public, we request a CVE ID (if not already issued) and publish details.

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

**Q: I run `v3.16` – will you patch it?**  
A: No. Upgrade to the latest monthly release.

**Q: Can I get early access to embargoed fixes?**  
A: Only project members under the disclosure agreement (see [SECURITY_CONTACTS](SECURITY_CONTACTS)) receive advance patches.

**Q: Where can I discuss the vulnerability once it is public?**  
A: Open/continue a GitHub issue **after** the advisory is published, or use `#kata-containers` on Slack with a link to the advisory.

---

*Last updated:* 2025-06-27
