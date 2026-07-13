# Generated iState VHDL Makefile
# Targets: build, run, verify, clean

GHDL ?= ghdl
GHDL_FLAGS ?= --std=08

SRC = runtime.vhd harness_tb.vhd
TOP = SctdPhilosophersLedTop_tb

.PHONY: all build run verify clean

all: build

build: $(SRC)
	$(GHDL) -a $(GHDL_FLAGS) runtime.vhd
	$(GHDL) -a $(GHDL_FLAGS) harness_tb.vhd
	$(GHDL) -e $(GHDL_FLAGS) $(TOP)

run: build
	$(GHDL) -r $(GHDL_FLAGS) $(TOP) --stop-time=10ms

verify: run
	@echo "Comparing output.trace.txt with vectors/golden.trace.txt..."
	@diff -q output.trace.txt vectors/golden.trace.txt && echo "PASS" || (echo "FAIL: traces differ"; exit 1)

clean:
	rm -f *.o *.cf $(TOP) output.trace.txt work-obj08.cf