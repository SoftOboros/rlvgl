# sctd_philosophers_led.sdc - timing constraints for 12 MHz SCTD HDL proof.

create_clock -name clk_12mhz -period 83.333 [get_ports {clk_12mhz}]
