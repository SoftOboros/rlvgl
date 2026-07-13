<!--
TRUSTED-PUBLISHING.md - Operator setup for crates.io OIDC publication.
-->

# crates.io Trusted Publishing Setup

rlvgl release workflows authenticate to crates.io with short-lived GitHub OIDC
credentials. They do not read a long-lived `CARGO_REGISTRY_TOKEN` secret.

## Per-crate configuration

Trusted Publishers are configured on each crate, not on the account token
page. For every crate printed by:

```bash
scripts/publish_changed.sh --print-order
```

open:

```text
https://crates.io/crates/<crate-name>/settings
```

Add two GitHub Trusted Publisher entries with these values:

| Field | Primary publisher | Recovery publisher |
|---|---|---|
| Repository owner | `SoftOboros` | `SoftOboros` |
| Repository name | `rlvgl` | `rlvgl` |
| Workflow filename | `publish.yml` | `publish-continue.yml` |
| Environment | `release` | `release` |

Both entries are required. `publish.yml` performs the gated tag/manual
release. `publish-continue.yml` resumes a partially completed dependency chain
and uses the same publisher script.

The GitHub repository must also have a `release` Environment. Approval rules
on that Environment are recommended so the OIDC exchange cannot occur until a
release operator approves the job.

## Ownership requirement

The crates.io settings page requires direct crate ownership. Verify each crate
before release:

```bash
cargo owner --list <crate-name>
```

If access is inherited only through a GitHub team, add the release operator as
a named owner before configuring Trusted Publishing. Team publish permission
alone may not grant access to the crate settings page.

## First-publish bootstrap

crates.io cannot configure a Trusted Publisher before a crate exists. A new
crate therefore has this one-time sequence:

1. complete all package and Gate P checks;
2. publish version `0.1.0` manually with a valid owner credential;
3. add both Trusted Publisher entries above; and
4. rerun `publish-continue.yml`, which skips the now-published version and
   resumes the dependency-ordered chain.

For v0.2.5, `ratatui-rlvgl 0.1.0` is the bootstrap crate. Do not add a secret
fallback to the workflow: the manual first publish is the only exceptional
step, and subsequent releases use OIDC like the other owned crates.

## Workflow contract

Both publishing jobs:

- run in the `release` Environment;
- grant `contents: read` and `id-token: write`;
- use `rust-lang/crates-io-auth-action@v1` to exchange the GitHub identity;
- pass its short-lived output to Cargo as `CARGO_REGISTRY_TOKEN`; and
- invoke `scripts/publish_changed.sh` with a quoted base SHA.

The last rule prevents shell glob expansion if a malformed manual input such
as `*` reaches the workflow. The script still validates that the resulting
value names a real commit before calculating the publish diff.
