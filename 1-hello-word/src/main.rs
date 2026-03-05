#![no_std]
#![no_main]

use ch32_hal as hal;
use hal::delay::Delay;
use hal::gpio::{Level, Output};
use panic_halt as _;

const LONG_ON_MS: u32 = 900;
const SHORT_ON_MS: u32 = 220;
const GAP_MS: u32 = 220;
const CYCLE_GAP_MS: u32 = 900;

#[hal::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    // openCH 赤菟 LED1 位于 PE11，低电平点亮（active-low）。
    let mut led = Output::new(p.PE11, Level::High, Default::default());

    let mut delay = Delay;

    loop {
        // 一长
        led.set_low();
        delay.delay_ms(LONG_ON_MS);
        led.set_high();
        delay.delay_ms(GAP_MS);

        // 两短（第一短）
        led.set_low();
        delay.delay_ms(SHORT_ON_MS);
        led.set_high();
        delay.delay_ms(GAP_MS);

        // 两短（第二短）
        led.set_low();
        delay.delay_ms(SHORT_ON_MS);
        led.set_high();
        delay.delay_ms(CYCLE_GAP_MS);
    }
}
