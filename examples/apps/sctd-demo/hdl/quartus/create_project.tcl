# create_project.tcl - Quartus project setup for SCTD philosophers LED proof.
#
# Usage:
#   quartus_sh -t create_project.tcl -device <QUARTUS_DEVICE>
#
# Optional:
#   copy pins.local.tcl.template to pins.local.tcl and edit the PIN_* values.

proc get_arg_value {name} {
    global argc argv
    for {set i 0} {$i < $argc} {incr i} {
        if {[lindex $argv $i] eq $name} {
            incr i
            if {$i < $argc} {
                return [lindex $argv $i]
            }
        }
    }
    return ""
}

set project_name "sctd_philosophers_led"
set top_entity "sctd_philosophers_led_quartus_top"
set device [get_arg_value "-device"]

if {$device eq "" && [info exists ::env(QUARTUS_DEVICE)]} {
    set device $::env(QUARTUS_DEVICE)
}

if {$device eq ""} {
    puts "ERROR: Missing Quartus device."
    puts "Run: quartus_sh -t create_project.tcl -device <QUARTUS_DEVICE>"
    puts "Example only: quartus_sh -t create_project.tcl -device 10M50DAF484C7G"
    exit 2
}

project_new $project_name -overwrite

set_global_assignment -name DEVICE $device
set_global_assignment -name TOP_LEVEL_ENTITY $top_entity

set here [file dirname [file normalize [info script]]]
set hdl_dir [file normalize [file join $here ".."]]
set generated_verilog [file join $hdl_dir "_generated" "istate-local-preview" "verilog" "runtime.v"]
set shim_verilog [file join $here "sctd_philosophers_led_quartus_top.v"]
set constraints_sdc [file join $here "sctd_philosophers_led.sdc"]

if {![file exists $generated_verilog]} {
    puts "ERROR: Missing generated Verilog: $generated_verilog"
    puts "Regenerate or copy _generated/istate-local-preview before running Quartus."
    project_close
    exit 3
}

set_global_assignment -name VERILOG_FILE $generated_verilog
set_global_assignment -name VERILOG_FILE $shim_verilog
set_global_assignment -name SDC_FILE $constraints_sdc

set pin_script [file join $here "pins.local.tcl"]
if {[file exists $pin_script]} {
    source $pin_script
} else {
    post_message -type warning "No pins.local.tcl found. The project can compile, but fitter pin placement is not constrained."
    post_message -type warning "Copy pins.local.tcl.template to pins.local.tcl and fill in board-specific pins."
}

execute_flow -compile
project_close
