coverage:
	cargo test --features std
	grcov . -s . --binary-path ./target/debug/ -t html --branch --ignore-not-existing -o coverage/
