//! Host simulator entry point for the read-only Renogy battery monitor.

use std::{
    cell::RefCell,
    env,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Read, Write},
    path::Path,
    rc::Rc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rlvgl_battery_monitor::{
    BatteryMonitorApp, BatteryPoller, BatteryTelemetry, DEFAULT_BATTERY_ADDRESSES, PollResult,
    TARGET_HEIGHT, TARGET_WIDTH,
};
use rlvgl_core::{application::Application, event::Event};
use rlvgl_platform::{
    BlitRect, BlitterRenderer, CpuBlitter, InputEvent, PixelFmt, Surface, WgpuDisplay,
};

const DEFAULT_WIDTH: usize = TARGET_WIDTH as usize;
const DEFAULT_HEIGHT: usize = TARGET_HEIGHT as usize;
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(750);
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(250);

/// Append-only CSV recorder for CRC-validated BMS telemetry.
struct CsvLogger {
    writer: BufWriter<File>,
}

impl CsvLogger {
    fn open(path: &Path) -> Result<Self, String> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|error| format!("open CSV {}: {error}", path.display()))?;
        let needs_header = file
            .metadata()
            .map_err(|error| format!("inspect CSV {}: {error}", path.display()))?
            .len()
            == 0;
        if !needs_header {
            let contents = fs::read_to_string(path)
                .map_err(|error| format!("read CSV header {}: {error}", path.display()))?;
            let header = contents.lines().next().unwrap_or_default();
            if header != CSV_HEADER.trim_end() {
                return Err(format!(
                    "CSV {} uses a different schema; choose a new --csv path",
                    path.display()
                ));
            }
        }
        let mut logger = Self {
            writer: BufWriter::new(file),
        };
        if needs_header {
            logger
                .writer
                .write_all(CSV_HEADER.as_bytes())
                .and_then(|()| logger.writer.flush())
                .map_err(|error| format!("write CSV header {}: {error}", path.display()))?;
        }
        Ok(logger)
    }

    fn write_reading(&mut self, reading: BatteryTelemetry) -> Result<(), String> {
        let timestamp_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("system time before Unix epoch: {error}"))?
            .as_millis();
        writeln!(self.writer, "{}", csv_row(timestamp_unix_ms, reading))
            .and_then(|()| self.writer.flush())
            .map_err(|error| format!("write CSV telemetry: {error}"))
    }
}

const CSV_HEADER: &str = "timestamp_unix_ms,address_hex,address,module_voltage_decivolts,current_centiamps,cell_count,cell_1_decivolts,cell_2_decivolts,cell_3_decivolts,cell_4_decivolts,cell_temperature_count,cell_temperature_1_decicelsius,cell_temperature_2_decicelsius,cell_temperature_3_decicelsius,remaining_capacity_milliamphours,total_capacity_milliamphours,status1,status2,status3,charge_discharge_status\n";

fn csv_row(timestamp_unix_ms: u128, reading: BatteryTelemetry) -> String {
    format!(
        "{timestamp_unix_ms},{:02X},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:04X},{:04X},{:04X},{:04X}",
        reading.address,
        reading.address,
        reading.module_voltage_decivolts,
        reading.current_centiamps,
        reading.cell_count,
        reading.cell_voltage_decivolts[0],
        reading.cell_voltage_decivolts[1],
        reading.cell_voltage_decivolts[2],
        reading.cell_voltage_decivolts[3],
        reading.cell_temperature_count,
        reading.cell_temperature_decicelsius[0],
        reading.cell_temperature_decicelsius[1],
        reading.cell_temperature_decicelsius[2],
        reading.remaining_capacity_milliamphours,
        reading.total_capacity_milliamphours,
        reading.status1,
        reading.status2,
        reading.status3,
        reading.charge_discharge_status,
    )
}

/// Configured host RS-485 Modbus RTU transport.
struct HostModbusPoller {
    port: Box<dyn serialport::SerialPort>,
    csv_logger: CsvLogger,
    poll_interval: Duration,
    next_poll: Instant,
}

impl HostModbusPoller {
    fn open(path: &str, poll_interval: Duration, csv_path: &Path) -> Result<Self, String> {
        let port = serialport::new(path, 9_600)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .timeout(RESPONSE_TIMEOUT)
            .open()
            .map_err(|error| format!("open {path}: {error}"))?;
        Ok(Self {
            port,
            csv_logger: CsvLogger::open(csv_path)?,
            poll_interval,
            next_poll: Instant::now(),
        })
    }

    fn transaction(&mut self, address: u8, start: u16, count: u16) -> Result<Vec<u8>, String> {
        let mut request = vec![address, 0x03];
        request.extend_from_slice(&start.to_be_bytes());
        request.extend_from_slice(&count.to_be_bytes());
        request.extend_from_slice(&modbus_crc(&request).to_le_bytes());
        self.port
            .clear(serialport::ClearBuffer::Input)
            .map_err(|error| error.to_string())?;
        self.port
            .write_all(&request)
            .map_err(|error| error.to_string())?;
        self.port.flush().map_err(|error| error.to_string())?;

        let mut header = [0u8; 3];
        self.port
            .read_exact(&mut header)
            .map_err(|error| error.to_string())?;
        if header[0] != address {
            return Err(format!("unexpected response address {:02X}", header[0]));
        }
        if header[1] == 0x83 {
            return Err(format!("Modbus exception {:02X}", header[2]));
        }
        if header[1] != 0x03 || header[2] != (count * 2) as u8 {
            return Err(String::from("unexpected Modbus response shape"));
        }
        let mut tail = vec![0u8; usize::from(header[2]) + 2];
        self.port
            .read_exact(&mut tail)
            .map_err(|error| error.to_string())?;
        let mut response = header.to_vec();
        response.extend_from_slice(&tail);
        let expected_crc =
            u16::from_le_bytes([response[response.len() - 2], response[response.len() - 1]]);
        if modbus_crc(&response[..response.len() - 2]) != expected_crc {
            return Err(String::from("response CRC mismatch"));
        }
        Ok(response[3..response.len() - 2].to_vec())
    }

    fn read_telemetry(&mut self, address: u8) -> Result<BatteryTelemetry, String> {
        let cells = self.transaction(address, 0x1388, 5)?;
        let temperatures = self.transaction(address, 0x1399, 4)?;
        let metrics = self.transaction(address, 0x13B2, 6)?;
        let status = self.transaction(address, 0x13F2, 4)?;
        let word = |bytes: &[u8], index: usize| -> u16 {
            u16::from_be_bytes([bytes[index * 2], bytes[index * 2 + 1]])
        };
        Ok(BatteryTelemetry {
            address,
            cell_count: word(&cells, 0),
            cell_voltage_decivolts: [
                word(&cells, 1),
                word(&cells, 2),
                word(&cells, 3),
                word(&cells, 4),
            ],
            cell_temperature_count: word(&temperatures, 0),
            cell_temperature_decicelsius: [
                word(&temperatures, 1) as i16,
                word(&temperatures, 2) as i16,
                word(&temperatures, 3) as i16,
            ],
            current_centiamps: word(&metrics, 0) as i16,
            module_voltage_decivolts: word(&metrics, 1),
            remaining_capacity_milliamphours: u32::from(word(&metrics, 2)) << 16
                | u32::from(word(&metrics, 3)),
            total_capacity_milliamphours: u32::from(word(&metrics, 4)) << 16
                | u32::from(word(&metrics, 5)),
            status1: word(&status, 0),
            status2: word(&status, 1),
            status3: word(&status, 2),
            charge_discharge_status: word(&status, 3),
        })
    }
}

impl BatteryPoller for HostModbusPoller {
    fn poll(&mut self, address: u8) -> PollResult {
        if Instant::now() < self.next_poll {
            return PollResult::Pending;
        }
        self.next_poll = Instant::now() + self.poll_interval;
        match self.read_telemetry(address) {
            Ok(telemetry) => match self.csv_logger.write_reading(telemetry) {
                Ok(()) => PollResult::Telemetry(telemetry),
                Err(error) => PollResult::Error(error),
            },
            Err(error) => PollResult::Error(error),
        }
    }
}

fn modbus_crc(bytes: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for byte in bytes {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}

fn main() {
    let mut port = None;
    let mut width = DEFAULT_WIDTH;
    let mut height = DEFAULT_HEIGHT;
    let mut poll_interval = DEFAULT_POLL_INTERVAL;
    let mut csv_path = String::from("battery-monitor.csv");
    let mut once = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--port" => port = args.next(),
            "--poll-ms" => match args.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(value) if value > 0 => poll_interval = Duration::from_millis(value),
                _ => return usage("--poll-ms needs a positive integer"),
            },
            "--csv" => match args.next() {
                Some(path) if !path.is_empty() => csv_path = path,
                _ => return usage("--csv needs a file path"),
            },
            "--screen" => match args
                .next()
                .and_then(|value| value.split_once('x').map(|(w, h)| (w.parse(), h.parse())))
            {
                Some((Ok(w), Ok(h))) => {
                    width = w;
                    height = h;
                }
                _ => return usage("--screen needs WIDTHxHEIGHT"),
            },
            "--once" => once = true,
            "--help" | "-h" => return usage(""),
            _ => return usage(&format!("unknown argument {argument}")),
        }
    }
    let Some(port) = port else {
        return usage("--port is required");
    };
    let mut poller = match HostModbusPoller::open(&port, poll_interval, Path::new(&csv_path)) {
        Ok(poller) => poller,
        Err(error) => {
            eprintln!("Unable to start battery monitor: {error}");
            return;
        }
    };
    if once {
        for address in DEFAULT_BATTERY_ADDRESSES {
            match poller.read_telemetry(address) {
                Ok(reading) => {
                    if let Err(error) = poller.csv_logger.write_reading(reading) {
                        eprintln!("{:02X}: {error}", address);
                        continue;
                    }
                    println!(
                        "{:02X}: {}.{:01} V, {:+}.{:02} A, cells {}.{}/{}.{}/{}.{}/{}.{}, cell T {}.{}/{}.{}/{}.{}, {} / {} Ah, status {:04X}/{:04X}/{:04X}/{:04X}",
                        address,
                        reading.module_voltage_decivolts / 10,
                        reading.module_voltage_decivolts % 10,
                        reading.current_centiamps / 100,
                        reading.current_centiamps.unsigned_abs() % 100,
                        reading.cell_voltage_decivolts[0] / 10,
                        reading.cell_voltage_decivolts[0] % 10,
                        reading.cell_voltage_decivolts[1] / 10,
                        reading.cell_voltage_decivolts[1] % 10,
                        reading.cell_voltage_decivolts[2] / 10,
                        reading.cell_voltage_decivolts[2] % 10,
                        reading.cell_voltage_decivolts[3] / 10,
                        reading.cell_voltage_decivolts[3] % 10,
                        reading.cell_temperature_decicelsius[0] / 10,
                        reading.cell_temperature_decicelsius[0].unsigned_abs() % 10,
                        reading.cell_temperature_decicelsius[1] / 10,
                        reading.cell_temperature_decicelsius[1].unsigned_abs() % 10,
                        reading.cell_temperature_decicelsius[2] / 10,
                        reading.cell_temperature_decicelsius[2].unsigned_abs() % 10,
                        reading.remaining_capacity_milliamphours / 1000,
                        reading.total_capacity_milliamphours / 1000,
                        reading.status1,
                        reading.status2,
                        reading.status3,
                        reading.charge_discharge_status,
                    )
                }
                Err(error) => eprintln!("{:02X}: {error}", address),
            }
        }
        return;
    }
    let mut application = BatteryMonitorApp::new(poller, DEFAULT_BATTERY_ADDRESSES);
    let root = Rc::new(RefCell::new(application.build(width as u32, height as u32)));
    let application: Rc<RefCell<dyn Application>> = Rc::new(RefCell::new(application));
    application.borrow_mut().after_event(&root, &Event::Tick);

    let frame_callback = {
        let root = root.clone();
        let application = application.clone();
        move |frame: &mut [u8], frame_width: usize, frame_height: usize| {
            application.borrow_mut().tick(&root);
            let mut blitter = CpuBlitter;
            let surface = Surface::new(
                frame,
                frame_width * 4,
                PixelFmt::Argb8888,
                frame_width as u32,
                frame_height as u32,
            );
            let mut renderer: BlitterRenderer<'_, CpuBlitter, 16> =
                BlitterRenderer::new(&mut blitter, surface);
            root.borrow().draw(&mut renderer);
            renderer.planner().add(BlitRect {
                x: 0,
                y: 0,
                w: frame_width as u32,
                h: frame_height as u32,
            });
        }
    };
    WgpuDisplay::new(width, height).run(frame_callback, {
        let root = root.clone();
        let application = application.clone();
        move |event: InputEvent| {
            root.borrow_mut().dispatch_event(&event);
            application.borrow_mut().after_event(&root, &event);
        }
    });
}

fn usage(problem: &str) {
    if !problem.is_empty() {
        eprintln!("{problem}");
    }
    eprintln!(
        "Usage: rlvgl-battery-monitor-sim --port /dev/cu.usbserial-... [--csv battery-monitor.csv] [--poll-ms 750] [--screen 800x480] [--once]"
    );
}

#[cfg(test)]
mod tests {
    use super::modbus_crc;

    #[test]
    fn crc_matches_verified_module_voltage_request() {
        assert_eq!(modbus_crc(&[0xf7, 0x03, 0x13, 0xb3, 0x00, 0x01]), 0xff65);
    }
}
