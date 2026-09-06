//! Shared ESP-HAL runner for the portable rlvgl network-time application.
//!
//! Board entry points own only chip initialization and typed GPIO selection.
//! This module owns the shared SSD1306, STTS22H, Wi-Fi, smoltcp, and application
//! lifecycle. Network configuration, NVS, SNTP validation, UTC conversion, and
//! holdover live in reusable rlvgl crates.

use core::cell::RefCell;

use embedded_hal_bus::i2c::RefCellDevice;
use esp_hal::{
    Blocking,
    delay::Delay,
    i2c::master::I2c,
    peripherals::{RADIO_CLK, RNG, TIMG0, WIFI},
    rng::Rng,
    time::{self, Duration, Instant},
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{
    config::PowerSaveMode,
    wifi::{ClientConfiguration, Configuration},
};
use rlvgl_app_network_time::{
    ClockReading, DisplayState, NetworkTimeApp, NetworkTimeModel, TARGET_HEIGHT, TARGET_WIDTH,
};
use rlvgl_core::{WidgetNode, application::Application, renderer::Renderer};
use rlvgl_device_stts22h::{Averaging, Config as SensorConfig, Stts22h};
use rlvgl_network::{
    ConnectionState, HoldoverClock, NetworkTime, NtpError, RetryPolicy, WifiCredentials,
    load_or_seed, ntp_request, parse_ntp_response,
};
use rlvgl_platform::{Ssd1306Display, display::DisplayDriver};
use smoltcp::{
    iface::{
        Config as InterfaceConfig, Interface, PollResult, SocketHandle, SocketSet, SocketStorage,
    },
    phy::Device,
    socket::{
        dhcpv4,
        udp::{self, PacketMetadata},
    },
    wire::{EthernetAddress, HardwareAddress, IpAddress, IpCidr, Ipv4Address},
};
use ssd1306::{
    I2CDisplayInterface, Ssd1306,
    prelude::{DisplayRotation, DisplaySize128x64},
};

/// Heap required by the rlvgl tree and ESP Wi-Fi runtime.
pub(crate) const HEAP_SIZE: usize = 72 * 1024;
const NTP_PORT: u16 = 123;
const LOCAL_UDP_PORT: u16 = 49_152;
const NTP_TIMEOUT: Duration = Duration::from_secs(5);
const RESYNC_INTERVAL: Duration = Duration::from_secs(60 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(10);
const ASSOCIATION_TIMEOUT: Duration = Duration::from_secs(20);
const DISPLAY_ADDRESS: u8 = 0x3c;
const WIFI_RETRY_POLICY: RetryPolicy = RetryPolicy::new(5, 250, 4_000);

// These values are optional provisioning seeds. Once a seed has been written
// to NVS, later firmware builds can omit both environment variables.
const WIFI_SSID_SEED: &str = match option_env!("RLVGL_WIFI_SSID") {
    Some(value) => value,
    None => "",
};
const WIFI_PASSWORD_SEED: &str = match option_env!("RLVGL_WIFI_PASSWORD") {
    Some(value) => value,
    None => "",
};

/// Chip peripheral tokens consumed by the common ESP Wi-Fi runtime.
pub(crate) struct WifiPeripherals {
    /// Timer group used by the ESP Wi-Fi scheduler.
    pub(crate) timer_group: TIMG0,
    /// Hardware random-number source used by ESP Wi-Fi and smoltcp.
    pub(crate) rng: RNG,
    /// Radio clock-control token.
    pub(crate) radio_clock: RADIO_CLK,
    /// Wi-Fi peripheral token.
    pub(crate) wifi: WIFI,
}

#[derive(Clone, Copy)]
enum SyncError {
    Socket,
    Timeout,
    InvalidPacket(NtpError),
}

impl core::fmt::Debug for SyncError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Socket => formatter.write_str("Socket"),
            Self::Timeout => formatter.write_str("Timeout"),
            Self::InvalidPacket(error) => {
                formatter.debug_tuple("InvalidPacket").field(error).finish()
            }
        }
    }
}

struct Network<'a, D>
where
    D: Device,
{
    interface: Interface,
    device: D,
    sockets: SocketSet<'a>,
    dhcp_handle: SocketHandle,
    udp_handle: SocketHandle,
    configured: bool,
}

impl<'a, D> Network<'a, D>
where
    D: Device,
{
    fn poll(&mut self) {
        while self
            .interface
            .poll(smoltcp_now(), &mut self.device, &mut self.sockets)
            != PollResult::None
        {}

        let event = self
            .sockets
            .get_mut::<dhcpv4::Socket>(self.dhcp_handle)
            .poll();
        match event {
            Some(dhcpv4::Event::Configured(config)) => {
                let address = config.address;
                let router = config.router;
                self.interface.update_ip_addrs(|addresses| {
                    addresses.clear();
                    let _ = addresses.push(IpCidr::Ipv4(address));
                });
                self.interface.routes_mut().remove_default_ipv4_route();
                if let Some(router) = router {
                    let _ = self.interface.routes_mut().add_default_ipv4_route(router);
                }
                self.configured = true;
                println!("DHCP configured: {:?}, router {:?}", address, router);
            }
            Some(dhcpv4::Event::Deconfigured) => {
                self.interface
                    .update_ip_addrs(|addresses| addresses.clear());
                self.interface.routes_mut().remove_default_ipv4_route();
                self.configured = false;
                println!("DHCP configuration lost");
            }
            None => {}
        }
    }

    fn is_up(&self) -> bool {
        self.configured
    }

    fn send(&mut self, destination: IpAddress, port: u16, payload: &[u8]) -> Result<(), ()> {
        self.poll();
        self.sockets
            .get_mut::<udp::Socket>(self.udp_handle)
            .send_slice(payload, (destination, port))
            .map_err(|_| ())
    }

    fn receive(&mut self, payload: &mut [u8]) -> Option<(usize, IpAddress, u16)> {
        self.poll();
        self.sockets
            .get_mut::<udp::Socket>(self.udp_handle)
            .recv_slice(payload)
            .ok()
            .map(|(length, metadata)| (length, metadata.endpoint.addr, metadata.endpoint.port))
    }
}

/// Run the display, persistent configuration, network, and 1 Hz application.
pub(crate) fn run(i2c: I2c<'static, Blocking>, peripherals: WifiPeripherals) -> ! {
    let delay = Delay::new();

    let shared_i2c = RefCell::new(i2c);
    let interface =
        I2CDisplayInterface::new_custom_address(RefCellDevice::new(&shared_i2c), DISPLAY_ADDRESS);
    let raw = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    let mut display = Ssd1306Display::new(raw).expect("ssd1306 init");

    let mut app = NetworkTimeApp::new();
    let display_model = app.model();
    let display_root = app.build(TARGET_WIDTH, TARGET_HEIGHT);

    delay.delay_millis(12_u32);
    let mut sensor = Stts22h::new(RefCellDevice::new(&shared_i2c));
    let sensor_present = match sensor
        .probe()
        .and_then(|()| sensor.configure(SensorConfig::low_odr(Averaging::Samples8)))
    {
        Ok(()) => {
            println!("STTS22H detected at 0x38; 1 Hz low-ODR enabled");
            true
        }
        Err(error) => {
            println!("STTS22H unavailable at 0x38: {error:?}");
            false
        }
    };

    let seed = if WIFI_SSID_SEED.is_empty() {
        None
    } else {
        match WifiCredentials::new(WIFI_SSID_SEED, WIFI_PASSWORD_SEED) {
            Ok(credentials) => Some(credentials),
            Err(error) => {
                println!("Invalid Wi-Fi provisioning seed: {error:?}");
                display_model.set(DisplayState::ConfigurationFailure);
                paint(&mut display, &display_root);
                park(&delay);
            }
        }
    };

    let mut config_store = match rlvgl_network_esp_nvs::open_store() {
        Ok(store) => store,
        Err(error) => {
            println!("Network configuration storage unavailable: {error:?}");
            display_model.set(DisplayState::StorageFailure);
            paint(&mut display, &display_root);
            park(&delay);
        }
    };
    let resolved = match load_or_seed(&mut config_store, seed) {
        Ok(config) => config,
        Err(error) => {
            println!("Network configuration load/store failed: {error:?}");
            display_model.set(DisplayState::StorageFailure);
            paint(&mut display, &display_root);
            park(&delay);
        }
    };
    drop(config_store);

    let Some(resolved) = resolved else {
        println!("No stored Wi-Fi configuration and no provisioning seed");
        display_model.set(DisplayState::AwaitingCredentials);
        paint(&mut display, &display_root);
        park(&delay);
    };
    println!(
        "Wi-Fi configuration {:?}, generation {}, SSID {:?}",
        resolved.origin,
        resolved.config.generation(),
        resolved.config.credentials().ssid()
    );
    display_model.set(DisplayState::Connection {
        state: ConnectionState::Stored,
        elapsed_seconds: 0,
    });
    paint(&mut display, &display_root);

    display_model.set(DisplayState::Connection {
        state: ConnectionState::RadioStarting,
        elapsed_seconds: 0,
    });
    paint(&mut display, &display_root);

    let timer_group = TimerGroup::new(peripherals.timer_group);
    let mut rng = Rng::new(peripherals.rng);
    let wifi_init =
        esp_wifi::init(timer_group.timer0, rng, peripherals.radio_clock).expect("wifi init");
    let (mut controller, interfaces) =
        esp_wifi::wifi::new(&wifi_init, peripherals.wifi).expect("wifi device");
    let mut device = interfaces.sta;
    let network_interface = create_interface(&mut device, rng.random());

    // Buffers precede the socket set so they outlive the sockets borrowing them.
    let mut socket_storage: [SocketStorage; 2] = Default::default();
    let mut udp_rx_meta = [PacketMetadata::EMPTY; 1];
    let mut udp_rx_buffer = [0_u8; 64];
    let mut udp_tx_meta = [PacketMetadata::EMPTY; 1];
    let mut udp_tx_buffer = [0_u8; 64];
    let mut sockets = SocketSet::new(&mut socket_storage[..]);
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());
    let mut udp_socket = udp::Socket::new(
        udp::PacketBuffer::new(&mut udp_rx_meta[..], &mut udp_rx_buffer[..]),
        udp::PacketBuffer::new(&mut udp_tx_meta[..], &mut udp_tx_buffer[..]),
    );
    udp_socket.bind(LOCAL_UDP_PORT).expect("udp bind");
    let udp_handle = sockets.add(udp_socket);
    let mut network = Network {
        interface: network_interface,
        device,
        sockets,
        dhcp_handle,
        udp_handle,
        configured: false,
    };

    controller
        .set_power_saving(PowerSaveMode::None)
        .expect("wifi power mode");
    let credentials = resolved.config.credentials();
    let station = Configuration::Client(ClientConfiguration {
        ssid: credentials
            .ssid()
            .try_into()
            .expect("validated Wi-Fi SSID capacity"),
        password: credentials
            .password()
            .try_into()
            .expect("validated Wi-Fi password capacity"),
        ..Default::default()
    });
    controller
        .set_configuration(&station)
        .expect("wifi configuration");
    controller.start().expect("wifi start");

    if !wait_for_wifi(
        &mut controller,
        &display_model,
        &display_root,
        &mut display,
        &delay,
    ) {
        park(&delay);
    }
    wait_for_dhcp(
        &mut network,
        &display_model,
        &display_root,
        &mut display,
        &delay,
    );

    let mut clock = loop {
        display_model.set(DisplayState::Synchronizing);
        paint(&mut display, &display_root);
        match sync_from_cloudflare(&mut network, &delay) {
            Ok((sample, rtt_millis)) => {
                println!(
                    "SNTP synchronized: stratum {}, round trip {} ms",
                    sample.stratum, rtt_millis
                );
                break HoldoverClock::from_sntp(sample, monotonic_millis(), rtt_millis);
            }
            Err(error) => {
                println!("SNTP attempt failed: {error:?}");
                display_model.set(DisplayState::SyncRetry { delay_seconds: 5 });
                paint(&mut display, &display_root);
                service_for(&mut network, &delay, Duration::from_secs(5));
            }
        }
    };

    let mut next_sync = Instant::now() + RESYNC_INTERVAL;
    let mut next_connect = Instant::now();
    let mut last_displayed_second = u64::MAX;

    loop {
        network.poll();
        let now = Instant::now();
        let wifi_connected = controller.is_connected().unwrap_or(false);

        if !wifi_connected && now >= next_connect {
            println!("Wi-Fi disconnected; requesting reconnect");
            let _ = controller.connect();
            next_connect = now + RECONNECT_INTERVAL;
        }

        if wifi_connected && network.is_up() && now >= next_sync {
            match sync_from_cloudflare(&mut network, &delay) {
                Ok((sample, rtt_millis)) => {
                    clock.resynchronize(sample, monotonic_millis(), rtt_millis);
                    next_sync = Instant::now() + RESYNC_INTERVAL;
                    println!(
                        "SNTP resynchronized: stratum {}, round trip {} ms",
                        sample.stratum, rtt_millis
                    );
                }
                Err(error) => {
                    println!("SNTP resync failed: {error:?}");
                    next_sync = Instant::now() + RETRY_INTERVAL;
                }
            }
        }

        // A resync can block for an SNTP timeout, so sample the monotonic clock
        // again before calculating holdover time.
        let display_millis = monotonic_millis();
        let current_second = clock.unix_millis_at(display_millis) / 1_000;
        if current_second != last_displayed_second {
            let temperature_centidegrees = if sensor_present {
                match sensor.read_temperature() {
                    Ok(value) => Some(value.centi_celsius()),
                    Err(error) => {
                        println!("STTS22H temperature read failed: {error:?}");
                        None
                    }
                }
            } else {
                None
            };
            display_model.set(DisplayState::Clock(ClockReading {
                unix_seconds: current_second,
                connected: wifi_connected && network.is_up(),
                sync_age_seconds: clock.sync_age_seconds(display_millis),
                temperature_centidegrees,
            }));
            paint(&mut display, &display_root);
            last_displayed_second = current_second;
        }

        delay.delay_millis(10_u32);
    }
}

fn wait_for_wifi<D>(
    controller: &mut esp_wifi::wifi::WifiController<'_>,
    model: &NetworkTimeModel,
    root: &WidgetNode,
    display: &mut D,
    delay: &Delay,
) -> bool
where
    D: DisplayDriver + Renderer,
{
    for attempt in 1..=WIFI_RETRY_POLICY.max_attempts() {
        println!("Wi-Fi connection attempt {attempt}");
        let _ = controller.connect();
        let started = Instant::now();
        let deadline = started + ASSOCIATION_TIMEOUT;
        let mut displayed_second = u64::MAX;

        while Instant::now() < deadline {
            if controller.is_connected().unwrap_or(false) {
                println!("Wi-Fi connected");
                return true;
            }
            let second = (Instant::now() - started).as_secs();
            if second != displayed_second {
                model.set(DisplayState::Connection {
                    state: ConnectionState::Associating { attempt },
                    elapsed_seconds: second as u32,
                });
                paint(display, root);
                displayed_second = second;
            }
            delay.delay_millis(20_u32);
        }

        if let Some(backoff) = WIFI_RETRY_POLICY.delay_after_failure(attempt) {
            println!("Wi-Fi attempt {attempt} timed out; retry in {backoff} ms");
            delay.delay_millis(backoff);
        }
    }

    println!("Wi-Fi association attempt budget exhausted");
    model.set(DisplayState::Connection {
        state: ConnectionState::Failed,
        elapsed_seconds: 0,
    });
    paint(display, root);
    false
}

fn wait_for_dhcp<D, R>(
    network: &mut Network<'_, D>,
    model: &NetworkTimeModel,
    root: &WidgetNode,
    display: &mut R,
    delay: &Delay,
) where
    D: Device,
    R: DisplayDriver + Renderer,
{
    let started = Instant::now();
    let mut displayed_second = u64::MAX;
    loop {
        network.poll();
        if network.is_up() {
            model.set(DisplayState::Connection {
                state: ConnectionState::GotIp,
                elapsed_seconds: 0,
            });
            paint(display, root);
            return;
        }
        let second = (Instant::now() - started).as_secs();
        if second != displayed_second {
            model.set(DisplayState::Connection {
                state: ConnectionState::AcquiringAddress,
                elapsed_seconds: second as u32,
            });
            paint(display, root);
            displayed_second = second;
        }
        delay.delay_millis(10_u32);
    }
}

fn sync_from_cloudflare<D>(
    network: &mut Network<'_, D>,
    delay: &Delay,
) -> Result<(NetworkTime, u64), SyncError>
where
    D: Device,
{
    // Cloudflare publishes both IPv4 anycast endpoints for time.cloudflare.com.
    let servers = [
        IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 1)),
        IpAddress::Ipv4(Ipv4Address::new(162, 159, 200, 123)),
    ];
    let mut last_error = SyncError::Timeout;
    for server in servers {
        match ntp_exchange(network, delay, server) {
            Ok(sample) => return Ok(sample),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

fn ntp_exchange<D>(
    network: &mut Network<'_, D>,
    delay: &Delay,
    server: IpAddress,
) -> Result<(NetworkTime, u64), SyncError>
where
    D: Device,
{
    let mut response = [0_u8; 64];
    while network.receive(&mut response).is_some() {}

    let request = ntp_request();
    let started = Instant::now();
    network
        .send(server, NTP_PORT, &request)
        .map_err(|_| SyncError::Socket)?;
    let deadline = started + NTP_TIMEOUT;

    while Instant::now() < deadline {
        if let Some((length, source, source_port)) = network.receive(&mut response)
            && source == server
            && source_port == NTP_PORT
        {
            let sample =
                parse_ntp_response(&response[..length]).map_err(SyncError::InvalidPacket)?;
            let rtt_millis = (Instant::now() - started).as_millis();
            return Ok((sample, rtt_millis));
        }
        delay.delay_millis(10_u32);
    }

    Err(SyncError::Timeout)
}

fn service_for<D>(network: &mut Network<'_, D>, delay: &Delay, duration: Duration)
where
    D: Device,
{
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        network.poll();
        delay.delay_millis(10_u32);
    }
}

fn create_interface(device: &mut esp_wifi::wifi::WifiDevice<'_>, random_seed: u32) -> Interface {
    let mut config = InterfaceConfig::new(HardwareAddress::Ethernet(EthernetAddress::from_bytes(
        &device.mac_address(),
    )));
    config.random_seed = u64::from(random_seed);
    Interface::new(config, device, smoltcp_now())
}

fn smoltcp_now() -> smoltcp::time::Instant {
    smoltcp::time::Instant::from_micros(
        time::Instant::now().duration_since_epoch().as_micros() as i64
    )
}

fn monotonic_millis() -> u64 {
    Instant::now().duration_since_epoch().as_millis()
}

fn paint<D>(display: &mut D, root: &WidgetNode)
where
    D: DisplayDriver + Renderer,
{
    root.draw(display);
    DisplayDriver::vsync(display);
}

fn park(delay: &Delay) -> ! {
    loop {
        delay.delay_millis(1_000_u32);
    }
}
