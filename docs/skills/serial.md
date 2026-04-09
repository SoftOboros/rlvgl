# /serial

Read debug serial output from STM32H747I-DISCO UART via ST-LINK VCP.

## Usage

```
/serial [timeout_seconds]
```

Default timeout is 5 seconds.

## What it does

Runs `tools/serial_monitor.py` to capture USART1 output from the board.

## Playit wire protocol

The serial port accepts `rlvgl-playit` commands (see `playit/README.md`):

| Command | Description |
|---------|-------------|
| `?` | Status/telemetry |
| `T<x>,<y>` | Inject tap |
| `PD<x>,<y>` / `PM<x>,<y>` / `PU<x>,<y>` | Raw pointer events |
| `MT<n>:<id>,<s>,<x>,<y>;...` | Multi-touch (s=D/U/C) |
| `KD:<key>` / `KU:<key>` | Key down/up |
| `T@<tag>:<x>,<y>` | Tap tagged widget |
| `QB:<tag>` / `QE:<tag>` / `QC:<tag>` | Query widget state |
| `D<x>,<y>,<w>,<h>[,<frames>]` | Framebuffer dump |
| `RS` / `RE` / `RD` | Event recorder start/stop/dump |

## Troubleshooting

If no output is received:
1. Is the board running? (green LED)
2. Is the USB cable connected?
3. Is USART1 configured in the firmware?
