#![no_std]
#![no_main]

#[cfg(not(target_arch = "xtensa"))]
compile_error!("请使用 --target xtensa-esp32s3-none-elf 构建固件");

esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

use alloc::string::ToString;
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_5X8, FONT_6X10},
    },
    pixelcolor::Rgb565,
    prelude::*,
    text::{Baseline, Text},
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    main,
    rng::Rng,
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Duration as HalDuration, Instant as HalInstant, Rate},
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{
    init,
    wifi::{AuthMethod, ClientConfiguration, Configuration as WifiConfiguration, WifiController, new},
};
use esp32s3 as _;
use lesson_3_codex_panel::{IDLE_TEXT, PanelSnapshot, ProtocolParser, TITLE_TEXT, parse_ipv4_address};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ST7735s,
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
};
use smoltcp::{
    iface::{Config as NetConfig, Interface, SocketHandle, SocketSet, SocketStorage},
    socket::{dhcpv4, tcp},
    time::Instant as NetInstant,
    wire::{EthernetAddress, HardwareAddress, IpCidr, Ipv4Address},
};

const SPI_FREQUENCY_MHZ: u32 = 20;
const PANEL_FRAMEBUFFER_WIDTH: u16 = 80;
const PANEL_FRAMEBUFFER_HEIGHT: u16 = 160;
const DISPLAY_OFFSET_X: u16 = 26;
const DISPLAY_OFFSET_Y: u16 = 1;
const BACKLIGHT_ACTIVE_HIGH: bool = true;
const STATUS_LED_ACTIVE_LOW: bool = true;
const TITLE_X: i32 = 4;
const TITLE_Y: i32 = 4;
const BODY_X: i32 = 4;
const BODY_Y: i32 = 18;
const BODY_PITCH: i32 = 10;
const BLINK_INTERVAL_MS: u32 = 500;
const WIFI_HEAP_BYTES: usize = 72 * 1024;
const WIFI_RETRY_MS: u32 = 3_000;
const TCP_RETRY_MS: u32 = 2_000;
const TCP_HEARTBEAT_MS: u32 = 2_000;
const TCP_LOCAL_PORT: u16 = 49_500;
const TCP_BUFFER_BYTES: usize = 1_024;
const TCP_READ_CHUNK_BYTES: usize = 256;
const BOARD_HELLO: &[u8] = b"HELLO esp32-panel\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkState {
    WifiJoin,
    DhcpLease,
    TcpDial,
    TcpReady,
}

impl LinkState {
    fn status_label(self, active: bool) -> &'static str {
        if active {
            "RUNNING"
        } else if matches!(self, Self::TcpReady) {
            "IDLE"
        } else {
            "LINK"
        }
    }

    fn body_text(self) -> &'static str {
        match self {
            Self::WifiJoin => "WiFi join...",
            Self::DhcpLease => "DHCP lease...",
            Self::TcpDial => "TCP dial...",
            Self::TcpReady => IDLE_TEXT,
        }
    }
}

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = BusyDelay;

    esp_alloc::heap_allocator!(size: WIFI_HEAP_BYTES);

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(SPI_FREQUENCY_MHZ))
            .with_mode(Mode::_0),
    )
    .expect("SPI2 配置应有效")
    .with_sck(peripherals.GPIO12)
    .with_mosi(peripherals.GPIO11)
    .with_miso(peripherals.GPIO13);

    let dc = Output::new(peripherals.GPIO40, Level::Low, OutputConfig::default());
    let rst = Output::new(peripherals.GPIO38, Level::High, OutputConfig::default());
    let cs = Output::new(peripherals.GPIO39, Level::High, OutputConfig::default());
    let mut backlight = Output::new(
        peripherals.GPIO41,
        backlight_level(false),
        OutputConfig::default(),
    );
    let mut status_led = Output::new(
        peripherals.GPIO1,
        indicator_level(false),
        OutputConfig::default(),
    );

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).expect("CS 初始电平应可拉高");
    let mut di_buffer = [0_u8; 512];
    let di = SpiInterface::new(spi_device, dc, &mut di_buffer);

    let mut display = Builder::new(ST7735s, di)
        .reset_pin(rst)
        .display_size(PANEL_FRAMEBUFFER_WIDTH, PANEL_FRAMEBUFFER_HEIGHT)
        .display_offset(DISPLAY_OFFSET_X, DISPLAY_OFFSET_Y)
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .expect("LCD 初始化失败");

    set_backlight(&mut backlight, true);

    let mut parser = ProtocolParser::new();
    let mut snapshot = PanelSnapshot::default();
    let mut screen_dirty = true;
    let mut blink_on = false;
    let mut last_blink_toggle = HalInstant::now();
    let mut link_state = LinkState::WifiJoin;

    let server_ip = server_ip();
    let server_port = server_port();
    println!(
        "lesson 3: wifi ssid='{}' server={}.{}.{}.{}:{}",
        wifi_ssid(),
        server_ip[0],
        server_ip[1],
        server_ip[2],
        server_ip[3],
        server_port
    );

    draw_panel(&mut display, &snapshot, link_state).expect("启动页绘制失败");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let rng = Rng::new(peripherals.RNG);
    let wifi_init = init(timg0.timer0, rng).expect("esp-wifi 初始化失败");
    let (mut wifi, interfaces) = new(&wifi_init, peripherals.WIFI).expect("WiFi 控制器创建失败");

    configure_wifi(&mut wifi);
    wifi.start().expect("WiFi 启动失败");
    wifi.connect().expect("WiFi 首次连接失败");

    let mut wifi_device = interfaces.sta;
    let mac = wifi_device.mac_address();
    let mut iface_config = NetConfig::new(HardwareAddress::Ethernet(EthernetAddress(mac)));
    iface_config.random_seed = u64::from_be_bytes([mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], 0, 1]);
    let boot_instant = HalInstant::now();
    let mut iface = Interface::new(iface_config, &mut wifi_device, network_instant(boot_instant));

    let mut socket_storage = [SocketStorage::EMPTY, SocketStorage::EMPTY];
    let mut sockets = SocketSet::new(&mut socket_storage[..]);
    let dhcp_handle = sockets.add(dhcpv4::Socket::new());

    let mut tcp_rx_buffer = [0_u8; TCP_BUFFER_BYTES];
    let mut tcp_tx_buffer = [0_u8; TCP_BUFFER_BYTES];
    let tcp_socket = tcp::Socket::new(
        tcp::SocketBuffer::new(&mut tcp_rx_buffer[..]),
        tcp::SocketBuffer::new(&mut tcp_tx_buffer[..]),
    );
    let tcp_handle = sockets.add(tcp_socket);

    let mut hello_sent = false;
    let mut last_wifi_retry = HalInstant::now();
    let mut last_tcp_retry = HalInstant::now();
    let mut last_ping_at = HalInstant::now();

    loop {
        let now = HalInstant::now();
        ensure_wifi_connected(&mut wifi, &mut link_state, &mut last_wifi_retry, now);

        let net_now = network_instant(boot_instant);
        let _ = iface.poll(net_now, &mut wifi_device, &mut sockets);

        if handle_dhcp_event(&mut iface, &mut sockets, dhcp_handle, &mut link_state) {
            hello_sent = false;
            screen_dirty = true;
        }

        drive_tcp_client(
            &mut iface,
            &mut sockets,
            tcp_handle,
            server_ip,
            server_port,
            &mut hello_sent,
            &mut link_state,
            &mut last_tcp_retry,
            &mut last_ping_at,
            &mut parser,
            &mut snapshot,
            &mut screen_dirty,
            now,
        );

        update_indicator(
            &mut status_led,
            snapshot.active,
            &mut blink_on,
            &mut last_blink_toggle,
        );

        if screen_dirty {
            draw_panel(&mut display, &snapshot, link_state).expect("面板绘制失败");
            screen_dirty = false;
        }
    }
}

fn configure_wifi(wifi: &mut WifiController<'_>) {
    let configuration = WifiConfiguration::Client(ClientConfiguration {
        ssid: wifi_ssid().to_string(),
        password: wifi_password().to_string(),
        auth_method: wifi_auth_method(),
        ..Default::default()
    });

    wifi.set_configuration(&configuration)
        .expect("WiFi 配置失败");
}

fn wifi_ssid() -> &'static str {
    option_env!("CODEX_PANEL_WIFI_SSID").unwrap_or("Xiaomi_2E16")
}

fn wifi_password() -> &'static str {
    option_env!("CODEX_PANEL_WIFI_PASSWORD").unwrap_or("fangtang1234")
}

fn wifi_auth_method() -> AuthMethod {
    if wifi_password().is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    }
}

fn server_ip() -> [u8; 4] {
    let ip_text = option_env!("CODEX_PANEL_SERVER_IP").unwrap_or("192.168.31.52");
    parse_ipv4_address(ip_text).expect("CODEX_PANEL_SERVER_IP 必须是合法 IPv4")
}

fn server_port() -> u16 {
    option_env!("CODEX_PANEL_SERVER_PORT")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(31_337)
}

fn network_instant(boot_instant: HalInstant) -> NetInstant {
    NetInstant::from_millis(boot_instant.elapsed().as_millis() as i64)
}

fn ensure_wifi_connected(
    wifi: &mut WifiController<'_>,
    link_state: &mut LinkState,
    last_retry: &mut HalInstant,
    now: HalInstant,
) {
    match wifi.is_connected() {
        Ok(true) => {
            if matches!(*link_state, LinkState::WifiJoin) {
                *link_state = LinkState::DhcpLease;
            }
        }
        _ => {
            *link_state = LinkState::WifiJoin;
            if last_retry.elapsed() >= HalDuration::from_millis(WIFI_RETRY_MS.into()) {
                println!("lesson 3: retry wifi connect");
                if wifi.connect().is_ok() {
                    *last_retry = now;
                }
            }
        }
    }
}

fn handle_dhcp_event(
    iface: &mut Interface,
    sockets: &mut SocketSet<'_>,
    dhcp_handle: SocketHandle,
    link_state: &mut LinkState,
) -> bool {
    match sockets.get_mut::<dhcpv4::Socket>(dhcp_handle).poll() {
        Some(dhcpv4::Event::Configured(config)) => {
            iface.update_ip_addrs(|addrs| {
                addrs.clear();
                addrs.push(IpCidr::Ipv4(config.address)).ok();
            });

            if let Some(router) = config.router {
                iface.routes_mut().add_default_ipv4_route(router).ok();
            } else {
                iface.routes_mut().remove_default_ipv4_route();
            }

            println!("lesson 3: dhcp ip={}", config.address);
            *link_state = LinkState::TcpDial;
            true
        }
        Some(dhcpv4::Event::Deconfigured) => {
            iface.update_ip_addrs(|addrs| addrs.clear());
            iface.routes_mut().remove_default_ipv4_route();
            println!("lesson 3: dhcp lost lease");
            *link_state = LinkState::DhcpLease;
            true
        }
        None => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn drive_tcp_client(
    iface: &mut Interface,
    sockets: &mut SocketSet<'_>,
    tcp_handle: SocketHandle,
    server_ip: [u8; 4],
    server_port: u16,
    hello_sent: &mut bool,
    link_state: &mut LinkState,
    last_tcp_retry: &mut HalInstant,
    last_ping_at: &mut HalInstant,
    parser: &mut ProtocolParser,
    snapshot: &mut PanelSnapshot,
    screen_dirty: &mut bool,
    now: HalInstant,
) {
    let has_ipv4 = iface.ipv4_addr().is_some();
    let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);

    if !has_ipv4 {
        if socket.is_open() {
            socket.abort();
        }
        *hello_sent = false;
        *link_state = LinkState::DhcpLease;
        return;
    }

    if !socket.is_open()
        && last_tcp_retry.elapsed() >= HalDuration::from_millis(TCP_RETRY_MS.into())
    {
        let remote = (
            Ipv4Address::new(server_ip[0], server_ip[1], server_ip[2], server_ip[3]),
            server_port,
        );

        match socket.connect(iface.context(), remote, TCP_LOCAL_PORT) {
            Ok(()) => {
                println!("lesson 3: tcp dial...");
                *last_tcp_retry = now;
                *link_state = LinkState::TcpDial;
            }
            Err(error) => {
                println!("lesson 3: tcp connect error: {:?}", error);
                *last_tcp_retry = now;
            }
        }
    }

    if socket.is_active() {
        *link_state = LinkState::TcpReady;
    } else if has_ipv4 {
        *link_state = LinkState::TcpDial;
    }

    if socket.may_send() {
        if !*hello_sent {
            if socket.send_slice(BOARD_HELLO).is_ok() {
                *hello_sent = true;
                *last_ping_at = now;
            }
        } else if last_ping_at.elapsed() >= HalDuration::from_millis(TCP_HEARTBEAT_MS.into())
        {
            let mut ping_buffer = [0_u8; 32];
            let ping_len = write_ping_line(&mut ping_buffer, last_tcp_retry.elapsed().as_millis());
            if ping_len > 0 && socket.send_slice(&ping_buffer[..ping_len]).is_ok() {
                *last_ping_at = now;
            }
        }
    }

    while socket.can_recv() {
        let mut chunk = [0_u8; TCP_READ_CHUNK_BYTES];
        let Ok(size) = socket.recv_slice(&mut chunk) else {
            break;
        };

        for &byte in &chunk[..size] {
            if let Some(next_snapshot) = parser.push_byte(byte) {
                *snapshot = next_snapshot;
                *screen_dirty = true;
            }
        }
    }

    if !socket.is_open() {
        *hello_sent = false;
        if has_ipv4 {
            *link_state = LinkState::TcpDial;
        }
    }
}

fn write_ping_line(buffer: &mut [u8], uptime_ms: u64) -> usize {
    if buffer.len() < 8 {
        return 0;
    }

    let prefix = b"PING ";
    buffer[..prefix.len()].copy_from_slice(prefix);

    let mut digits = [0_u8; 20];
    let digits_len = write_decimal_ascii(&mut digits, uptime_ms);
    if prefix.len() + digits_len + 1 > buffer.len() {
        return 0;
    }

    buffer[prefix.len()..prefix.len() + digits_len].copy_from_slice(&digits[..digits_len]);
    buffer[prefix.len() + digits_len] = b'\n';
    prefix.len() + digits_len + 1
}

fn write_decimal_ascii(buffer: &mut [u8], mut value: u64) -> usize {
    if buffer.is_empty() {
        return 0;
    }

    if value == 0 {
        buffer[0] = b'0';
        return 1;
    }

    let mut digits = [0_u8; 20];
    let mut count = 0usize;
    while value > 0 && count < digits.len() {
        digits[count] = b'0' + (value % 10) as u8;
        value /= 10;
        count += 1;
    }

    for index in 0..count {
        buffer[index] = digits[count - index - 1];
    }

    count
}

fn draw_panel<D>(display: &mut D, snapshot: &PanelSnapshot, link_state: LinkState) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    display.clear(Rgb565::BLACK)?;

    let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CYAN);
    let body_style = MonoTextStyle::new(&FONT_5X8, Rgb565::WHITE);
    let accent_style = MonoTextStyle::new(&FONT_5X8, Rgb565::GREEN);

    Text::with_baseline(
        TITLE_TEXT,
        Point::new(TITLE_X, TITLE_Y),
        title_style,
        Baseline::Top,
    )
    .draw(display)?;

    Text::with_baseline(
        link_state.status_label(snapshot.active),
        Point::new(112, TITLE_Y + 1),
        accent_style,
        Baseline::Top,
    )
    .draw(display)?;

    if snapshot.count == 0 {
        Text::with_baseline(
            link_state.body_text(),
            Point::new(BODY_X, BODY_Y),
            body_style,
            Baseline::Top,
        )
        .draw(display)?;
        return Ok(());
    }

    for index in 0..snapshot.count {
        let y = BODY_Y + index as i32 * BODY_PITCH;
        Text::with_baseline(
            snapshot.title(index),
            Point::new(BODY_X, y),
            body_style,
            Baseline::Top,
        )
        .draw(display)?;
    }

    Ok(())
}

fn update_indicator(
    led: &mut Output<'_>,
    active: bool,
    blink_on: &mut bool,
    last_blink_toggle: &mut HalInstant,
) {
    if !active {
        *blink_on = false;
        set_indicator(led, false);
        return;
    }

    if last_blink_toggle.elapsed() >= HalDuration::from_millis(BLINK_INTERVAL_MS.into()) {
        *blink_on = !*blink_on;
        *last_blink_toggle = HalInstant::now();
    }

    set_indicator(led, *blink_on);
}

fn set_indicator(pin: &mut Output<'_>, enabled: bool) {
    match indicator_level(enabled) {
        Level::High => pin.set_high(),
        Level::Low => pin.set_low(),
    }
}

fn indicator_level(enabled: bool) -> Level {
    if enabled == STATUS_LED_ACTIVE_LOW {
        Level::Low
    } else {
        Level::High
    }
}

fn set_backlight(pin: &mut Output<'_>, enabled: bool) {
    match backlight_level(enabled) {
        Level::High => pin.set_high(),
        Level::Low => pin.set_low(),
    }
}

fn backlight_level(enabled: bool) -> Level {
    if enabled == BACKLIGHT_ACTIVE_HIGH {
        Level::High
    } else {
        Level::Low
    }
}

struct BusyDelay;

impl DelayNs for BusyDelay {
    fn delay_ns(&mut self, ns: u32) {
        let start = HalInstant::now();
        let delay = HalDuration::from_micros(ns.div_ceil(1_000) as u64);

        while start.elapsed() < delay {
            core::hint::spin_loop();
        }
    }
}
