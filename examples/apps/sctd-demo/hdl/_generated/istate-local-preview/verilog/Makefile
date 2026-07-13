# Generated iState Verilog Makefile
# Targets: build, run, verify, clean

# Simulator selection (iverilog by default, can override with VSIM=vcs|xsim|modelsim)
VSIM ?= iverilog

SRC = runtime.v harness_tb.v
TOP = SctdPhilosophersLedTop_tb

.PHONY: all build run verify clean

all: build

ifeq ($(VSIM),iverilog)
# Icarus Verilog
build: $(SRC)
	iverilog -g2012 -o $(TOP).vvp $(SRC)

run: build
	vvp $(TOP).vvp

else ifeq ($(VSIM),vcs)
# Synopsys VCS
build: $(SRC)
	vcs -sverilog -o $(TOP) $(SRC)

run: build
	./$(TOP)

else ifeq ($(VSIM),xsim)
# Xilinx Vivado Simulator
build: $(SRC)
	xvlog --sv $(SRC)
	xelab -debug typical $(TOP) -s $(TOP)_sim

run: build
	xsim $(TOP)_sim -runall

else
# ModelSim/QuestaSim
build: $(SRC)
	vlib work
	vlog -sv $(SRC)

run: build
	vsim -c -do "run -all; quit" $(TOP)
endif

verify: run
	@echo "Comparing output.trace.txt with vectors/golden.trace.txt..."
	@diff -q output.trace.txt vectors/golden.trace.txt && echo "PASS" || (echo "FAIL: traces differ"; exit 1)

clean:
	rm -f *.vvp *.vcd *.log output.trace.txt
	rm -rf work xsim.dir .Xil *.wdb *.jou
	rm -f $(TOP) simv csrc