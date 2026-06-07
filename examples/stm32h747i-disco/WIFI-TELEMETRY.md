# WiFi Telemetry (Future)

## Overview

Stream CPU stats and driver metrics over WiFi for remote monitoring.
Not yet implemented — this document captures the intended design and the
D3 SRAM telemetry layout that the `cpu_stats` feature already populates.

## D3 SRAM Telemetry Layout

The `cpu_stats` feature writes live instrumentation data to D3 SRAM.  These
addresses are readable by a probe (ST-LINK / probe-rs) and will be the source
for WiFi streaming when the transport is available.

| Address       | Field              | Writer | Notes                   |
|---------------|--------------------|--------|-------------------------|
| 0x3800_0800   | CM7 CPU %          | CM7    | 0–100                   |
| 0x3800_0804   | CM7 busy cycles    | CM7    | Per-frame busy count    |
| 0x3800_0808   | CM7 total cycles   | CM7    | Per-frame total count   |
| 0x3800_080C   | CM4 CPU %          | CM4    | 0–100                   |
| 0x3800_0810   | CM4 busy cycles    | CM4    | (reserved, not yet populated) |
| 0x3800_0814   | CM4 total cycles   | CM4    | (reserved, not yet populated) |
| 0x3800_0818   | DMA2D cycles       | CM7    | Stub — future subsystem timing |
| 0x3800_081C   | Touch poll cycles  | CM7    | Stub — future subsystem timing |
| 0x3800_0820   | Serial poll cycles | CM7    | Stub — future subsystem timing |
| 0x3800_0824   | WiFi stats         | —      | Reserved for WiFi module |

### Existing telemetry (pre-cpu_stats)

| Address       | Field              |
|---------------|--------------------|
| 0x3800_0600   | Main loop marker   |
| 0x3800_0604   | Event count        |
| 0x3800_0608   | Tick count         |
| 0x3800_060C   | Render count       |
| 0x3800_0660   | Loop heartbeat     |
| 0x3800_0700–07FF | Event ring (16 entries) |

## Planned Architecture

1. **WiFi module** — ESP32-C3 or similar, connected to CM4 via SPI or UART.
2. **Transport** — CM4 periodically reads the D3 SRAM telemetry block and
   transmits it.  UDP multicast is the simplest option; a lightweight HTTP
   endpoint is an alternative for browser-based dashboards.
3. **Network stack** — `smoltcp` (no-std TCP/IP) on CM4, or offload to the
   ESP32 firmware if using AT-command mode.
4. **Host-side tool** — CLI receiver that decodes the telemetry stream and
   renders a live dashboard (terminal TUI or web).

## Driver Metric Aggregation

The `CpuStats` struct exposes one-liner methods to record per-subsystem cycle
counts:

```rust
cpu_stats.record_dma2d_cycles(cycles);
cpu_stats.record_touch_cycles(cycles);
cpu_stats.record_serial_cycles(cycles);
```

These write directly to reserved D3 SRAM slots.  To instrument a driver,
bracket the operation with `cyccnt()` reads:

```rust
let t0 = cpu_stats.cyccnt();
// ... driver operation ...
let t1 = cpu_stats.cyccnt();
cpu_stats.record_dma2d_cycles(t1.wrapping_sub(t0));
```

The WiFi transport will include these values in the telemetry packet when
populated.

## Prerequisites

- WiFi hardware interface (SPI or UART pins, power enable GPIO)
- Driver crate for the WiFi module (ESP-AT or native)
- Network stack (`smoltcp` or ESP firmware offload)
- Telemetry packet format definition
