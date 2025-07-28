coverage:
	cargo test --workspace --target x86_64-unknown-linux-gnu
	grcov . -s . --binary-path ./target/x86_64-unknown-linux-gnu/debug/ -t html --branch --ignore-not-existing -o coverage/
