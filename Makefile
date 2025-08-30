coverage:
	cargo test --workspace --target x86_64-unknown-linux-gnu
	grcov . -s . --binary-path ./target/x86_64-unknown-linux-gnu/debug/ -t html --branch --ignore-not-existing -o coverage/

.PHONY: test-sim
test-sim:
	# Run headless simulator tests with required features
	cargo test --test headless --features "simulator qrcode png jpeg gif fontdue"

.PHONY: clippy-all
clippy-all:
	# Broad clippy pass across workspace with rich feature set
	cargo clippy --workspace --features "canvas,fatfs,fontdue,gif,jpeg,lottie,nes,png,pinyin,qrcode" -- -D warnings

# Sharded pre-commit phases
.PHONY: pre-commit-format pre-commit-clippy pre-commit-creator-cli pre-commit-creator-cli-fast pre-commit-creator-ui pre-commit-creator-ui-fast pre-commit-docs pre-commit-embedded pre-commit-fast pre-commit-all pre-commit

pre-commit-format:
	@echo "[phase 0] format"
	cargo fmt --all

pre-commit-clippy:
	@echo "[phase 1] clippy (core/workspace)"
	cargo clippy --workspace -- -D warnings

pre-commit-creator-cli:
	@echo "[phase 2] build+test: creator CLI"
	cargo build --bin rlvgl-creator --features creator
	cargo test --tests --features creator

.PHONY: pre-commit-creator-cli-fast
pre-commit-creator-cli-fast:
	@echo "[phase 2] build+test: creator CLI (fast)"
	cargo build --bin rlvgl-creator --features creator
	cargo test --tests --features creator -- \
	  --skip cli::fonts::tests::pack_generates_stable_bin_and_json \
	  --skip cli::apng::tests::apng_has_stable_output_and_timing \
	  --skip cli::scaffold::tests::scaffold_generates_expected_files \
	  --skip cli::svg::tests::svg_renders_with_stable_hash \
	  --skip cli::scaffold::tests::scaffold_cargo_publish_dry_run_succeeds \
	  --skip cli::scaffold::tests::scaffold_cargo_check_embed_and_vendor_succeed \
	  --skip roundtrip_snapshot

pre-commit-creator-ui:
	@echo "[phase 3] build+test: creator UI"
	cargo test --tests --features "creator creator_ui"

pre-commit-creator-ui-fast:
	@echo "[phase 3] build+test: creator UI (fast)"
	cargo test --tests --features "creator creator_ui" -- \
	  --skip ui 

pre-commit-docs:
	@echo "[phase 4] docs (nightly)"
	@export ARTIFACTS_INCLUDE_DIR="$(PWD)/scripts/artifacts/include"; \
	 export ARTIFACTS_LIB_DIR="$(PWD)/scripts/artifacts/lib"; \
	 export ARTIFACTS_LIB64_DIR="$$ARTIFACTS_LIB_DIR"; \
	 RUSTDOCFLAGS="--cfg docsrs --cfg nightly" cargo +nightly doc --no-deps

pre-commit-embedded:
	@echo "[phase 5] embedded example (stm32h747i-disco)"
	- RUSTFLAGS="" cargo build --target thumbv7em-none-eabihf --bin rlvgl-stm32h747i-disco --features stm32h747i_disco || \
	  echo "warning: embedded target build skipped or failed (toolchain/target may be missing)" >&2

# Fast path (no docs/embedded/UI)
pre-commit-fast: pre-commit-format pre-commit-clippy pre-commit-creator-cli-fast

# Full pre-commit
pre-commit-all: pre-commit-fast pre-commit-creator-ui pre-commit-docs pre-commit-embedded

# Alias
pre-commit: pre-commit-all
