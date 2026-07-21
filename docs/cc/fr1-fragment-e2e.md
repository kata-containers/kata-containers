# FR-1 signed policy fragments — end-to-end proof (developer guide)

This shows how a **verified signed policy fragment** measurably changes an authorization
decision at enforcement time, using only pieces that live in this repository. It is the
reproducible proof for FR-1 Iteration 1 (apply-to-live-engine + attested issuer trust root).

## Reusable pieces (in-tree)

| Piece | Path |
|---|---|
| Offline fragment signer / keygen | `src/agent/security-reference-monitor/examples/sign-fragment.rs` |
| `LoadPolicyFragment` client command | `src/tools/agent-ctl` (`LoadPolicyFragment`) |
| Demo base policy (exec gated on a fragment) | `src/kata-opa/fragment-demo.rego` |
| Fragment verifier + apply path | `security-reference-monitor/src/fragments.rs`, `src/agent/policy/src/policy.rs` (`apply_fragment_module`) |

## The model

- A fragment carries a **signed Rego module** whose package is under the reserved
  `agent_policy.fragments` namespace. The base policy is authored to consult
  `data.agent_policy.fragments.*` (see `fragment-demo.rego`, where `ExecProcessRequest`
  defaults to `false` and becomes `data.agent_policy.fragments.exec_allowed`).
- The guest authorizes issuers from **measured state**: a TOML file in the (measured)
  rootfs at `/etc/kata/fragment-issuers.toml`:
  ```toml
  require_receipt = true
  [[issuer]]
  id = "issuerA"
  ed25519_pubkey_hex = "<64 hex chars>"
  min_svn = 0
  ```
  With no such file / no issuers, every fragment is rejected (fail-closed).
- `LoadPolicyFragment` runs **verify → apply → commit** atomically: the fragment is
  verified (issuer signature, monotonic SVN, add-only, receipt), its module is merged into
  the live engine via `add_policy` (never `set_policy`, so the FR-12 one-shot lock holds),
  and only then is the store state committed.

## Reproduce

```bash
SRM=src/agent/security-reference-monitor
SIGNER=target/aarch64-unknown-linux-musl/release/examples/sign-fragment

# 1. Build the signer and agent-ctl.
( cd $SRM && cargo build --release --example sign-fragment --target aarch64-unknown-linux-musl )
( cd src/tools/agent-ctl && cargo build --release --target aarch64-unknown-linux-musl )

# 2. Issuer keypair; put the public key in the guest's /etc/kata/fragment-issuers.toml
$SIGNER gen-key      # -> private_key_hex=... public_key_hex=...

# 3. Author a fragment module and sign it.
printf 'package agent_policy.fragments\nexec_allowed := true\n' > /tmp/frag.rego
$SIGNER sign --issuer issuerA --svn 1 --receipt r1 --includes exec \
        --module /tmp/frag.rego --key <private_key_hex>   # -> signature_hex=...

# 4. Boot a strict guest with fragment-demo.rego as the base policy, then over vsock:
kata-agent-ctl connect --server-address vsock://<CID>:1024 \
  -c "LoadPolicyFragment issuer=issuerA svn=1 receipt=r1 includes=exec \
      module=/tmp/frag.rego sig=<signature_hex>"
```

Before the fragment, an exec is **denied**; after it, the same exec is **allowed**; a
fragment signed by an unauthorized key is **rejected**.

## Automated proof

An environment-specific runner drives the whole flow on a strict `kata-parma` pod
(deploy → exec denied → sign+load → exec allowed → wrong-key rejected): see
`fr1-fragment-attack.sh` in the confidential-runtime test harness. Deterministic
coverage of the same guarantees (no VM required) is in the unit tests:

- `security-reference-monitor/src/fragments.rs` — signature / SVN / add-only / receipt gates.
- `src/agent/policy/src/policy.rs` — `apply_fragment_module` flips a decision deny→allow and
  rejects out-of-namespace modules.
