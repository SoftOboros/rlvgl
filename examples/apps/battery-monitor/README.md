<!-- Battery-monitor emulator usage and ESP32 transport boundary. -->

# rlvgl Battery Monitor

This emulator-first application displays read-only Renogy BMS telemetry for the
three-address bank currently configured as `F7`, `04`, and `05`. Its default
window is the target display's 800x480 resolution. The retained Ratatui surface
uses three columns—one per battery—with individual telemetry and status rows.
It opens a configured host serial port at 9600 8N1 and sends only Modbus
function `03` requests. Each displayed reading validates the received Modbus
CRC.

Run it from the repository root:

```sh
cargo run -p rlvgl-battery-monitor --bin rlvgl-battery-monitor-sim -- \
  --port /dev/cu.usbserial-BG02P5I7
```

Use `--poll-ms 750` to set the interval between individual battery polls. A
full three-battery refresh therefore takes roughly three intervals.

The simulator appends one CSV row for every CRC-validated telemetry update.
By default it writes `battery-monitor.csv` in the current directory; use
`--csv /path/to/charge-log.csv` to choose another file. The file includes a
Unix-millisecond timestamp, address, cell voltages, cell temperatures, current,
capacities, and raw status words. A pre-temperature CSV has a different schema;
start a new file when upgrading to this version.

For a non-UI wiring and protocol smoke check, add `--once`; it reads each
configured address once and prints the CRC-validated telemetry, including the
first four cell-voltage reports, to stdout. The display uses those reports for
four compact text bars per battery; the bars span 3.0 V through 3.6 V.

## ESP32 preparation

The UI crate owns no operating-system serial port. It depends only on the
`BatteryPoller` trait from `src/lib.rs`; an ESP32 UART plus RS-485 direction
control implementation can supply that trait without changing the display,
telemetry model, address list, or read-only policy.
