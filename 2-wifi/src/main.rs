#![no_std]
#![no_main]

#[cfg(not(target_arch = "xtensa"))]
compile_error!("请使用 --target xtensa-esp32s3-none-elf 构建固件");

esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

use alloc::string::String;
use embedded_graphics::{
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_10X20, FONT_5X8, FONT_6X10},
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
    time::{Duration, Instant, Rate},
    timer::timg::TimerGroup,
};
use esp_println::println;
use esp_wifi::{
    init,
    wifi::{WifiMode, new},
};
use esp32s3 as _;
use lesson_2_wifi::{
    DIAGNOSTIC_MARKER_TEXT, MAX_SSID_CHARS, MAX_VISIBLE_SSIDS, TITLE_TEXT, diagnostic_marker_top_left,
    display_ssid, format_ssid_lines,
};
use mipidsi::{
    Builder,
    interface::SpiInterface,
    models::ST7735s,
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
};

const SPI_FREQUENCY_MHZ: u32 = 20;
const PANEL_FRAMEBUFFER_WIDTH: u16 = 80;
const PANEL_FRAMEBUFFER_HEIGHT: u16 = 160;
const DISPLAY_OFFSET_X: u16 = 26;
const DISPLAY_OFFSET_Y: u16 = 1;
const BACKLIGHT_ACTIVE_HIGH: bool = true;
const WIFI_HEAP_BYTES: usize = 72 * 1024;
const WIFI_SCAN_LIMIT: usize = 12;
const SHOW_BOOT_DIAGNOSTICS: bool = false;
const DIAGNOSTIC_BACKLIGHT_SETTLE_MS: u32 = 150;
const DIAGNOSTIC_COLOR_HOLD_MS: u32 = 300;
const DIAGNOSTIC_MARKER_HOLD_MS: u32 = 1_000;
const TITLE_X: i32 = 4;
const TITLE_Y: i32 = 4;
const LIST_X: i32 = 4;
const LIST_Y: i32 = 18;
const LIST_PITCH: i32 = 10;

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = BusyDelay;

    esp_alloc::heap_allocator!(size: WIFI_HEAP_BYTES);

    println!("lesson 2: boot");

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
    delay.delay_ms(DIAGNOSTIC_BACKLIGHT_SETTLE_MS);

    if SHOW_BOOT_DIAGNOSTICS {
        run_display_diagnostics(&mut display, &mut delay).expect("显示诊断页绘制失败");
    }

    draw_scanning_screen(&mut display).expect("扫描页绘制失败");

    println!("lesson 2: scanning wifi...");

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let rng = Rng::new(peripherals.RNG);
    let wifi_init = init(timg0.timer0, rng).expect("esp-wifi 初始化失败");
    let (mut wifi, _interfaces) = new(&wifi_init, peripherals.WIFI).expect("WiFi 控制器创建失败");

    wifi.set_mode(WifiMode::Sta).expect("WiFi 模式切换失败");
    wifi.start().expect("WiFi 启动失败");

    let mut access_points = wifi.scan_n(WIFI_SCAN_LIMIT).expect("WiFi 扫描失败");
    access_points.sort_unstable_by(|left, right| right.signal_strength.cmp(&left.signal_strength));

    let visible_count = usize::min(access_points.len(), MAX_VISIBLE_SSIDS);
    let mut ssid_refs = [""; MAX_VISIBLE_SSIDS];
    for (slot, access_point) in ssid_refs.iter_mut().zip(access_points.iter()) {
        *slot = access_point.ssid.as_str();
    }

    let rendered_lines = format_ssid_lines(&ssid_refs[..visible_count]);
    draw_wifi_results(&mut display, &rendered_lines).expect("WiFi 列表绘制失败");

    println!("lesson 2: found {} access points", access_points.len());
    for (index, access_point) in access_points.iter().take(MAX_VISIBLE_SSIDS).enumerate() {
        let rendered_ssid = display_ssid(access_point.ssid.as_str(), MAX_SSID_CHARS);
        println!(
            "#{}: ssid='{}', rssi={}, channel={}",
            index + 1,
            rendered_ssid,
            access_point.signal_strength,
            access_point.channel
        );
    }
    println!("lesson 2: wifi list rendered");

    loop {
        core::hint::spin_loop();
    }
}

fn draw_scanning_screen<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    display.clear(Rgb565::BLACK)?;

    let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CYAN);
    let body_style = MonoTextStyle::new(&FONT_5X8, Rgb565::WHITE);

    Text::with_baseline(
        TITLE_TEXT,
        Point::new(TITLE_X, TITLE_Y),
        title_style,
        Baseline::Top,
    )
    .draw(display)?;

    Text::with_baseline(
        "Scanning...",
        Point::new(LIST_X, LIST_Y),
        body_style,
        Baseline::Top,
    )
    .draw(display)?;

    Ok(())
}

fn run_display_diagnostics<D, DELAY>(display: &mut D, delay: &mut DELAY) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
    DELAY: DelayNs,
{
    // 黑屏尚未定位前，启动时先跑一轮“纯色页 + 大字”诊断。
    // 这样可以快速区分是背光链路、像素写入，还是后续 WiFi 逻辑导致的不可见。
    println!("lesson 2: display diagnostics start");

    draw_solid_frame(display, Rgb565::RED)?;
    delay.delay_ms(DIAGNOSTIC_COLOR_HOLD_MS);

    draw_solid_frame(display, Rgb565::GREEN)?;
    delay.delay_ms(DIAGNOSTIC_COLOR_HOLD_MS);

    draw_solid_frame(display, Rgb565::BLUE)?;
    delay.delay_ms(DIAGNOSTIC_COLOR_HOLD_MS);

    draw_diagnostic_marker(display)?;
    delay.delay_ms(DIAGNOSTIC_MARKER_HOLD_MS);

    println!("lesson 2: display diagnostics done");

    Ok(())
}

fn draw_solid_frame<D>(display: &mut D, color: Rgb565) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    display.clear(color)
}

fn draw_diagnostic_marker<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    display.clear(Rgb565::BLACK)?;

    let marker_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    Text::with_baseline(
        DIAGNOSTIC_MARKER_TEXT,
        diagnostic_marker_top_left(),
        marker_style,
        Baseline::Top,
    )
    .draw(display)?;

    Ok(())
}

fn draw_wifi_results<D>(display: &mut D, lines: &[String]) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Rgb565>,
{
    display.clear(Rgb565::BLACK)?;

    let title_style = MonoTextStyle::new(&FONT_6X10, Rgb565::CYAN);
    let item_style = MonoTextStyle::new(&FONT_5X8, Rgb565::WHITE);

    Text::with_baseline(
        TITLE_TEXT,
        Point::new(TITLE_X, TITLE_Y),
        title_style,
        Baseline::Top,
    )
    .draw(display)?;

    for (index, line) in lines.iter().enumerate() {
        let y = LIST_Y + index as i32 * LIST_PITCH;
        Text::with_baseline(
            line.as_str(),
            Point::new(LIST_X, y),
            item_style,
            Baseline::Top,
        )
        .draw(display)?;
    }

    Ok(())
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
        let start = Instant::now();
        let delay = Duration::from_micros(ns.div_ceil(1_000) as u64);

        while start.elapsed() < delay {
            core::hint::spin_loop();
        }
    }
}
