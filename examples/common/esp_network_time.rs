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
    wifi::{AuthMethod, ClientConfiguration, Configuration, WifiController},
};
use rlvgl_app_network_time::{
    ClockReading, DisplayState, NETWORKS_PER_PAGE, NetworkTimeApp, NetworkTimeModel, TARGET_HEIGHT,
    TARGET_WIDTH,
};
use rlvgl_core::{WidgetNode, application::Application, renderer::Renderer};
use rlvgl_device_stts22h::{Averaging, Config as SensorConfig, Stts22h};
use rlvgl_network::{
    ConnectionState, HoldoverClock, NetworkTime, NtpError, RetryPolicy, WifiAccessPoint,
    WifiCredentials, WifiScan, WifiSecurity, WifiSsid, load_or_seed, ntp_request,
    parse_ntp_response,
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
const DHCP_TIMEOUT: Duration = Duration::from_secs(20);
const SCAN_INTERVAL: Duration = Duration::from_secs(60);
const SCAN_CAPACITY: usize = 16;
const CLOCK_PAGE_SECONDS: u64 = 8;
const SCAN_PAGE_SECONDS: u64 = 4;
const DISPLAY_ADDRESS: u8 = 0x3c;
const DISPLAY_INIT_ATTEMPTS: u8 = 3;
const WIFI_RETRY_POLICY: RetryPolicy = RetryPolicy::new(5, 250, 4_000);
const OPEN_RETRY_POLICY: RetryPolicy = RetryPolicy::new(1, 0, 0);

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

    // Association changes invalidate leases, routes, and queued NTP datagrams.
    // In particular, a lease from the previous AP must not qualify a new AP.
    fn reset_link(&mut self) {
        let dhcp = self.sockets.get_mut::<dhcpv4::Socket>(self.dhcp_handle);
        dhcp.reset();
        let _ = dhcp.poll();
        self.interface
            .update_ip_addrs(|addresses| addresses.clear());
        self.interface.routes_mut().remove_default_ipv4_route();
        let udp = self.sockets.get_mut::<udp::Socket>(self.udp_handle);
        udp.close();
        udp.bind(LOCAL_UDP_PORT).expect("udp rebind");
        self.configured = false;
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

struct Presentation<'a, D> {
    model: &'a NetworkTimeModel,
    root: &'a WidgetNode,
    display: &'a mut D,
}

impl<D: DisplayDriver + Renderer> Presentation<'_, D> {
    fn show(&mut self, state: DisplayState) {
        if self.model.get() != state {
            self.model.set(state);
            paint(self.display, self.root);
        }
    }
}

#[derive(Default)]
struct ScanResults {
    access_points: WifiScan<SCAN_CAPACITY>,
    total: usize,
    failed: bool,
}

impl ScanResults {
    fn page(&self, page: usize, connected_ssid: Option<WifiSsid>) -> DisplayState {
        if self.failed {
            return DisplayState::ScanFailure;
        }
        let pages = self.access_points.len().div_ceil(NETWORKS_PER_PAGE).max(1);
        let page = page % pages;
        let mut entries = [None; NETWORKS_PER_PAGE];
        for (entry, ap) in entries
            .iter_mut()
            .zip(self.access_points.iter().skip(page * NETWORKS_PER_PAGE))
        {
            *entry = Some(*ap);
        }
        DisplayState::Networks {
            entries,
            page: page + 1,
            pages,
            total: self.total,
            connected_ssid,
        }
    }
}

struct NetworkTarget {
    ssid: WifiSsid,
    station: ClientConfiguration,
    open: bool,
}

impl NetworkTarget {
    fn saved(credentials: &WifiCredentials) -> Self {
        Self {
            ssid: WifiSsid::new(credentials.ssid()).expect("validated SSID"),
            station: ClientConfiguration {
                ssid: credentials.ssid().try_into().expect("validated SSID"),
                password: credentials
                    .password()
                    .try_into()
                    .expect("validated password"),
                auth_method: if credentials.password().is_empty() {
                    AuthMethod::None
                } else {
                    AuthMethod::WPA2Personal
                },
                ..Default::default()
            },
            open: false,
        }
    }

    fn discovered(ap: WifiAccessPoint) -> Self {
        Self {
            ssid: ap.ssid,
            station: ClientConfiguration {
                ssid: ap.ssid.as_str().try_into().expect("validated SSID"),
                bssid: Some(ap.bssid),
                channel: Some(ap.channel),
                auth_method: AuthMethod::None,
                ..Default::default()
            },
            open: true,
        }
    }

    fn status(&self, message: &'static str, elapsed_seconds: u32) -> DisplayState {
        DisplayState::NetworkStatus {
            ssid: self.ssid,
            open: self.open,
            message,
            elapsed_seconds,
        }
    }
}

/// Run the display, persistent configuration, network, and 1 Hz application.
pub(crate) fn run(i2c: I2c<'static, Blocking>, peripherals: WifiPeripherals) -> ! {
    let delay = Delay::new();

    let shared_i2c = RefCell::new(i2c);
    // A USB warm reset can interrupt a panel transaction. The pinned HAL
    // resets its I2C peripheral after a write error; give initialization a
    // bounded opportunity to retry instead of panicking on the first error.
    // This does not clear or conceal a persistent wiring/bus fault.
    let mut attempt = 0;
    let mut display = loop {
        attempt += 1;
        let interface = I2CDisplayInterface::new_custom_address(
            RefCellDevice::new(&shared_i2c),
            DISPLAY_ADDRESS,
        );
        let raw = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        match Ssd1306Display::new(raw) {
            Ok(display) => break display,
            Err(error) => {
                println!(
                    "SSD1306 init attempt {attempt}/{DISPLAY_INIT_ATTEMPTS} failed: {error:?}"
                );
                assert!(
                    attempt < DISPLAY_INIT_ATTEMPTS,
                    "ssd1306 initialization retry budget exhausted"
                );
                delay.delay_millis(25_u32);
            }
        }
    };

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

    if let Some(config) = &resolved {
        println!(
            "Wi-Fi configuration {:?}, generation {}, SSID {:?}",
            config.origin,
            config.config.generation(),
            config.config.credentials().ssid()
        );
    } else {
        println!("No stored Wi-Fi configuration; will scan and try explicitly open networks");
    }

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
    // Starting in station mode also permits scanning on an unprovisioned unit.
    let station = resolved
        .as_ref()
        .map(|config| NetworkTarget::saved(config.config.credentials()).station)
        .unwrap_or_default();
    controller
        .set_configuration(&Configuration::Client(station))
        .expect("wifi configuration");
    let mut ui = Presentation {
        model: &display_model,
        root: &display_root,
        display: &mut display,
    };
    let mut clock: Option<HoldoverClock> = None;

    loop {
        // Each selection run ends with the radio stopped, canceling unfinished
        // association. Scans and association must not overlap in this driver.
        network.reset_link();
        controller.start().expect("wifi restart before scan");
        ui.show(DisplayState::Scanning);
        let mut scan = scan_networks(&mut controller).unwrap_or(ScanResults {
            failed: true,
            ..Default::default()
        });
        ui.show(scan.page(0, None));
        service_for(&mut network, &delay, Duration::from_secs(3));

        let mut selected = None;
        // Saved configuration is always first. Discovered targets exist only
        // in RAM and never pass through load_or_seed or NetworkConfigStore.
        let saved = resolved
            .as_ref()
            .map(|config| NetworkTarget::saved(config.config.credentials()));
        let candidates = saved.into_iter().chain(
            scan.access_points
                .open_candidates()
                .copied()
                .map(NetworkTarget::discovered),
        );
        for target in candidates {
            if let Some((sample, rtt_millis)) =
                try_network(&mut controller, &mut network, &target, &mut ui, &delay)
            {
                clock = Some(HoldoverClock::from_sntp(
                    sample,
                    monotonic_millis(),
                    rtt_millis,
                ));
                selected = Some(target);
                break;
            }
            service_for(&mut network, &delay, Duration::from_secs(2));
        }

        if let Some(target) = selected {
            let mut next_sync = Instant::now() + RESYNC_INTERVAL;
            let mut next_scan = Instant::now() + SCAN_INTERVAL;
            let view_started = Instant::now();
            let mut displayed_second = u64::MAX;
            let mut address_lost_at = None;
            loop {
                network.poll();
                let now = Instant::now();
                if !controller.is_connected().unwrap_or(false) {
                    println!("Wi-Fi link lost; restarting saved-first selection");
                    break;
                }
                if network.is_up() {
                    address_lost_at = None;
                } else if now - *address_lost_at.get_or_insert(now) >= DHCP_TIMEOUT {
                    println!("DHCP lease lost and not recovered; restarting selection");
                    break;
                }
                if network.is_up() && now >= next_sync {
                    match sync_from_cloudflare(&mut network, &delay) {
                        Ok((sample, rtt_millis)) => {
                            clock
                                .as_mut()
                                .expect("accepted initial time")
                                .resynchronize(sample, monotonic_millis(), rtt_millis);
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
                if now >= next_scan {
                    // A short background scan leaves the current association
                    // in place. Never roam away from a working saved network.
                    if let Some(updated) = scan_networks(&mut controller) {
                        scan = updated;
                    }
                    next_scan = Instant::now() + SCAN_INTERVAL;
                }
                let second = monotonic_millis() / 1_000;
                if second != displayed_second {
                    let temperature = if sensor_present {
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
                    ui.show(rotating_view(
                        &scan,
                        clock.as_ref(),
                        Some(target.ssid),
                        network.is_up(),
                        temperature,
                        (Instant::now() - view_started).as_secs(),
                    ));
                    displayed_second = second;
                }
                delay.delay_millis(10_u32);
            }
            controller.stop().expect("wifi stop after link loss");
            network.reset_link();
            service_for(&mut network, &delay, RECONNECT_INTERVAL);
        } else {
            println!("No usable saved/open network; rescan in 60s (NVS unchanged)");
            controller
                .stop()
                .expect("wifi stop after failed candidates");
            network.reset_link();
            let started = Instant::now();
            let mut displayed_second = u64::MAX;
            while Instant::now() - started < SCAN_INTERVAL {
                let second = (Instant::now() - started).as_secs();
                if second != displayed_second {
                    let temperature = if sensor_present {
                        sensor
                            .read_temperature()
                            .ok()
                            .map(|value| value.centi_celsius())
                    } else {
                        None
                    };
                    ui.show(rotating_view(
                        &scan,
                        clock.as_ref(),
                        None,
                        false,
                        temperature,
                        second,
                    ));
                    displayed_second = second;
                }
                delay.delay_millis(10_u32);
            }
        }
    }
}

fn rotating_view(
    scan: &ScanResults,
    clock: Option<&HoldoverClock>,
    ssid: Option<WifiSsid>,
    connected: bool,
    temperature_centidegrees: Option<i16>,
    elapsed_seconds: u64,
) -> DisplayState {
    let pages = scan.access_points.len().div_ceil(NETWORKS_PER_PAGE).max(1) as u64;
    let clock_seconds = if clock.is_some() {
        CLOCK_PAGE_SECONDS
    } else {
        0
    };
    let phase = elapsed_seconds % (clock_seconds + pages * SCAN_PAGE_SECONDS);
    if phase < clock_seconds {
        let clock = clock.expect("clock page requires a time sample");
        let millis = monotonic_millis();
        DisplayState::Clock(ClockReading {
            unix_seconds: clock.unix_millis_at(millis) / 1_000,
            connected,
            sync_age_seconds: clock.sync_age_seconds(millis),
            temperature_centidegrees,
        })
    } else {
        scan.page(
            ((phase - clock_seconds) / SCAN_PAGE_SECONDS) as usize,
            if connected { ssid } else { None },
        )
    }
}

fn scan_networks(controller: &mut WifiController<'_>) -> Option<ScanResults> {
    match controller.scan_n::<SCAN_CAPACITY>() {
        Ok((access_points, total)) => {
            let mut scan = ScanResults {
                access_points: WifiScan::new(),
                total,
                failed: false,
            };
            println!(
                "Wi-Fi scan: {} APs found, retaining at most {}",
                total, SCAN_CAPACITY
            );
            for ap in access_points {
                let security = match ap.auth_method {
                    Some(AuthMethod::None) => WifiSecurity::Open,
                    Some(_) => WifiSecurity::Protected,
                    None => WifiSecurity::Unknown,
                };
                println!(
                    "  SSID {:?}, RSSI {} dBm, channel {}, auth {:?}",
                    ap.ssid.as_str(),
                    ap.signal_strength,
                    ap.channel,
                    ap.auth_method
                );
                if let Ok(ssid) = WifiSsid::new(ap.ssid.as_str()) {
                    scan.access_points.insert(WifiAccessPoint {
                        ssid,
                        bssid: ap.bssid,
                        channel: ap.channel,
                        rssi: ap.signal_strength,
                        security,
                    });
                }
            }
            Some(scan)
        }
        Err(error) => {
            println!("Wi-Fi scan failed: {error:?}");
            None
        }
    }
}

fn try_network<D: Device, R: DisplayDriver + Renderer>(
    controller: &mut WifiController<'_>,
    network: &mut Network<'_, D>,
    target: &NetworkTarget,
    ui: &mut Presentation<'_, R>,
    delay: &Delay,
) -> Option<(NetworkTime, u64)> {
    println!(
        "Trying {} network {:?}",
        if target.open { "open" } else { "saved" },
        target.ssid.as_str()
    );
    controller.stop().expect("wifi stop before candidate");
    network.reset_link();
    if let Err(error) = controller.set_configuration(&Configuration::Client(target.station.clone()))
    {
        println!("Wi-Fi configuration failed: {error:?}");
        ui.show(target.status("configuration failed", 0));
        controller
            .start()
            .expect("wifi restart after rejected config");
        return None;
    }
    controller.start().expect("wifi start for candidate");
    if !wait_for_wifi(controller, target, ui, delay) {
        return None;
    }
    network.reset_link();
    if !wait_for_dhcp(network, controller, target, ui, delay) {
        return None;
    }
    ui.show(target.status("SNTP test", 0));
    match sync_from_cloudflare(network, delay) {
        Ok((sample, rtt_millis)) => {
            println!(
                "Network usable: {:?}; SNTP stratum {}, round trip {} ms",
                target.ssid.as_str(),
                sample.stratum,
                rtt_millis
            );
            Some((sample, rtt_millis))
        }
        Err(error) => {
            println!(
                "SNTP test failed for {:?}: {error:?}; not assuming Internet access",
                target.ssid.as_str()
            );
            ui.show(target.status("no valid NTP reply", 0));
            None
        }
    }
}

fn wait_for_wifi<D: DisplayDriver + Renderer>(
    controller: &mut WifiController<'_>,
    target: &NetworkTarget,
    ui: &mut Presentation<'_, D>,
    delay: &Delay,
) -> bool {
    let retry = if target.open {
        OPEN_RETRY_POLICY
    } else {
        WIFI_RETRY_POLICY
    };
    for attempt in 1..=retry.max_attempts() {
        println!("Wi-Fi connection attempt {attempt}");
        if let Err(error) = controller.connect() {
            println!("Wi-Fi connect request failed: {error:?}");
        }
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
                ui.show(target.status("associating", second as u32));
                displayed_second = second;
            }
            delay.delay_millis(20_u32);
        }

        // Explicitly cancel a pending connection so another attempt or scan
        // cannot race an association that outlived our deadline.
        controller
            .stop()
            .expect("wifi stop after association timeout");
        controller
            .start()
            .expect("wifi restart after association timeout");
        if let Some(backoff) = retry.delay_after_failure(attempt) {
            println!("Wi-Fi attempt {attempt} timed out; retry in {backoff} ms");
            delay.delay_millis(backoff);
        }
    }

    println!("Wi-Fi association attempt budget exhausted");
    ui.show(target.status("association failed", 20));
    false
}

fn wait_for_dhcp<D: Device, R: DisplayDriver + Renderer>(
    network: &mut Network<'_, D>,
    controller: &WifiController<'_>,
    target: &NetworkTarget,
    ui: &mut Presentation<'_, R>,
    delay: &Delay,
) -> bool {
    let started = Instant::now();
    let mut displayed_second = u64::MAX;
    while Instant::now() - started < DHCP_TIMEOUT {
        network.poll();
        if !controller.is_connected().unwrap_or(false) {
            ui.show(target.status("link lost in DHCP", 0));
            return false;
        }
        if network.is_up() {
            return true;
        }
        let second = (Instant::now() - started).as_secs();
        if second != displayed_second {
            ui.show(target.status("DHCP address", second as u32));
            displayed_second = second;
        }
        delay.delay_millis(10_u32);
    }
    println!("DHCP timed out for {:?}", target.ssid.as_str());
    ui.show(target.status("DHCP timed out", 20));
    false
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
