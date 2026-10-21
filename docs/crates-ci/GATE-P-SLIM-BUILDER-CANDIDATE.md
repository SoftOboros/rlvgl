# Gate P slim builder candidate (unqualified)

This candidate changes the builder image only. It does not change the frozen
release source, nightly-cohort contract, Gate P commands, staged packages, or
publication dependency. Do not point a release workflow at it until the cold
hosted proof below passes and the resulting image is addressed by digest.

## Current source and measured cost

`Dockerfile` and `scripts/docker-build.sh` are the repository's image recipe
and local build command. No workflow in `.github/workflows/` builds or pushes
`docker.io/iraa/rlvgl:latest`. That tag is mutable. The image currently pulled
locally is `iraa/rlvgl@sha256:b1ccd0d452d3a471ef2940c147a789fbd293ece9d487708708c90ca95884a3bf`
(image ID `sha256:753648d9c584e3ab33dd05c17dfac1204670fdb5f32cfac1bd6a2a3128e42e65`,
5,513,961,875 uncompressed bytes). Its history contains an older global
`@openai/codex` installation absent from the frozen Dockerfile, so the exact
source commit of that published image is not established by this repository.

Read-only `docker image history` gives the useful layer bounds: 2.99 GB for
the native apt dependencies; 1.04 GB for a floating Rust nightly and
`cargo-binutils`; another 1.04 GB for a later recursive ownership change of
`/opt/rust`; 169 MB for Node; and 174 MB for yarn plus the obsolete Codex CLI.
These are image-layer sizes, not clean-run disk peaks. The known removable
duplication is the preinstalled floating nightly (the job installs its exact
nightly privately) and the separate ownership copy-up. The first candidate
keeps every apt dependency, Node, yarn, sccache, and the existing rustup and
Cargo-binutils executables. `rust-objcopy` and `cargo size` remain available
for embedded work. It does not claim that GTK, SDL, LLVM, ARM GCC, or other
native dependencies can be removed from a full Gate P builder.

On the C56 local qualification, source excluding `target` measured 4,598,692
KiB (4.386 GiB); the job-scoped nightly profile measured 2,402,116 KiB
(2.291 GiB); and `target` reached 28,003,352 KiB (26.706 GiB). The current
image, source, and profile alone total about 11.81 GiB before Docker/OS
metadata or build output. GitHub's public standard Ubuntu runner lists 14 GB
SSD (about 13.04 GiB). Publishing the same image to GHCR does not alter that
disk requirement.

## Bounded image and checkout sequence

1. Build `Dockerfile.gate-p-slim` locally only when the release builder has
   enough free space and no competing qualification. It uses the immutable
   current image only as a source for rustup proxies and Cargo binutils, then
   starts a fresh Ubuntu 24.04 final image. It copies no preinstalled nightly
   and creates the builder user before copying `/opt/rust/cargo/bin`, avoiding
   the recursive ownership copy-up. Record the build transcript, final image
   ID, digest, `docker image inspect .Size`, and `docker image history`.
2. Verify the final image's command inventory and build environment. The
   existing `scripts/setup-ci-env.sh prepare` must install the exact selected
   nightly in a new private profile and verify its official manifest, Cargo
   bytes, components, targets, and source/workflow binding. Check `cargo size`
   and `rust-objcopy` against that profile. Compare full Gate P command results
   with the current image before proposing workflow adoption.
3. Keep the full recursive checkout for this release. `vendor/micropython`
   alone uses about 2.296 GiB, mostly nested `lib/` submodules, but the
   current source custody iterates all 39 declared child repositories and
   requires each child's checked-out `HEAD`, clean status, and index tree to
   match its pin. Leaving nested MicroPython children uninitialized would
   fail that guard even though Gate P does not invoke `make mpy-host-test`.
   A future checkout reduction needs a separately reviewed pin-proof contract;
   its possible disk saving is not part of this candidate.
4. Only after image and checkout measurements, consider per-job dependency
   images. A simulator/Creator job needs GUI libraries, Node, and Xvfb;
   embedded work needs ARM tools; staging needs its generator/packaging tools.
   Remove a package from a variant only after its complete cold shard passes.

## Cold hosted proof before standard-runner adoption

Use a diagnostic workflow on the public repository's ordinary
`ubuntu-latest` runner, with no cache and with the candidate final image pinned
to its immutable GHCR digest. Bind one exact source SHA and one nightly cohort
artifact across every shard. Log `df -B1 /` before and after image pull,
checkout/submodule initialization, nightly preparation, each command group,
staging, artifact extraction, and each consumer. Sample minimum free bytes,
`du -sk` for source/target/nightly/temp/Cargo caches, elapsed time, and cgroup
anonymous memory and OOM events during each shard. A cold job must complete
all its original checks and retain enough disk headroom under the existing
release guard; success with warmed caches is not proof.

The current source's whole-workspace Clippy and tests and the 31-crate ordered
Stage31 are not proved to fit a standard runner merely by moving other checks
to separate jobs. The three consumers may fan out only from one verified,
content-hashed staged-package set, with local patch paths reconstructed and
staged-source assertions retained. An aggregate Gate P job must verify every
required shard receipt, source SHA, cohort manifest, artifact digest and
publish-order completeness before publication is enabled. If any cold shard
exceeds the 14 GB disk or the job timeout, route that shard to the measured
larger/self-hosted runner; do not reduce the gate.

Official specifications: <https://docs.github.com/en/actions/reference/runners/github-hosted-runners>
and <https://docs.github.com/en/actions/reference/runners/larger-runners>.
