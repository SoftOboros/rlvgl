CARGO ?= cargo

all:
	$(CARGO) build --release

run:
	$(CARGO) run --bin harness -- vectors/events.txt

verify:
	$(CARGO) run --bin harness -- vectors/events.txt vectors/golden.trace.txt

verify-all:
	for f in $$(ls vectors/sequences/*.events.txt 2>/dev/null); do \
	  g=$${f%.events.txt}.golden.trace.txt; \
	  echo "Verifying $$f"; $(CARGO) run --bin harness -- "$$f" "$$g" || exit $$?; \
	done; echo PASS

.PHONY: all run verify