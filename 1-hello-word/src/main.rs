#![no_std]
#![no_main]

use ch32_hal as hal;
use hal::gpio::{Level, Output};
use panic_halt as _;

const SPIN_CYCLES: u32 = 1_200_000;

#[inline(never)]
fn busy_delay(cycles: u32) {
    // 最小依赖延时：不依赖定时器外设，纯 CPU 自旋，用于启动级排障。
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}

#[hal::entry]
fn main() -> ! {
    let p = hal::init(Default::default());

    // openCH 赤菟板载 LED 为低电平点亮，使用双灯交替可快速判断程序是否运行。
    let mut led1 = Output::new(p.PE11, Level::High, Default::default());
    let mut led2 = Output::new(p.PE12, Level::High, Default::default());

    loop {
        led1.set_low();
        led2.set_high();
        busy_delay(SPIN_CYCLES);

        led1.set_high();
        led2.set_low();
        busy_delay(SPIN_CYCLES);
    }
}
