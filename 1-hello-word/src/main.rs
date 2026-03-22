#![no_std]
#![no_main]

#[cfg(not(target_arch = "xtensa"))]
compile_error!("请使用 --target xtensa-esp32s3-none-elf 构建固件");

esp_bootloader_esp_idf::esp_app_desc!();

use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_10X20},
    pixelcolor::Rgb565,
    prelude::*,
    text::{Baseline, Text},
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Level, Output, OutputConfig},
    main,
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::{Duration, Instant, Rate},
};
use esp_println::println;
use esp32s3 as _;
use lesson_1_hello_word::{HELLO_WORLD_TEXT, hello_world_top_left};
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

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut delay = BusyDelay;

    println!("lesson 1: boot");

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

    // 正点原子板级示例的 `lcd_display_dir(1)` 最终等价于：
    // 1. 横屏显示
    // 2. MADCTL = 0xA8 -> `Deg270 + Bgr`
    // 3. 显存窗口固定偏移 `x + 1 / y + 26`
    //
    // `mipidsi` 的 `display_size/display_offset` 需要使用控制器默认竖屏
    // framebuffer 坐标系。对这块 0.96" ST7735S 来说，要先写成 `80x160`
    // 和 `(26, 1)`，再由 `Deg270` 旋转成最终逻辑上的 `160x80`。
    let mut display = Builder::new(ST7735s, di)
        .reset_pin(rst)
        .display_size(PANEL_FRAMEBUFFER_WIDTH, PANEL_FRAMEBUFFER_HEIGHT)
        .display_offset(DISPLAY_OFFSET_X, DISPLAY_OFFSET_Y)
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .expect("LCD 初始化失败");

    display.clear(Rgb565::BLACK).expect("清屏失败");

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    Text::with_baseline(
        HELLO_WORLD_TEXT,
        hello_world_top_left(),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .expect("文字绘制失败");

    // 参考页正文和示例代码对背光极性有冲突。
    // 最终以上板结果为准：当前这块板子需要 GPIO41 拉高才能点亮背光。
    set_backlight(&mut backlight, true);

    println!("lesson 1: hello world rendered");

    loop {
        core::hint::spin_loop();
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
        let start = Instant::now();
        let delay = Duration::from_micros(ns.div_ceil(1_000) as u64);

        while start.elapsed() < delay {
            core::hint::spin_loop();
        }
    }
}
