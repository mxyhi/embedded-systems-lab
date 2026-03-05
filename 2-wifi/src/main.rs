#![no_std]
#![no_main]

use ch32_hal as hal;
use hal::delay::Delay;
use hal::gpio::{Level, Output};
use hal::mode::Blocking;
use hal::peripherals;
use hal::usart::{self, Uart, UartRx, UartTx};
use hal::println;
use opench_rust_wifi::contains_token;
use panic_halt as _;

const WIFI_BAUDRATE: u32 = 115_200;
const RESPONSE_BUF_SIZE: usize = 256;
const RESPONSE_IDLE_TIMEOUT_MS: u32 = 600;
const POLL_INTERVAL_MS: u32 = 1;

fn drain_rx(rx: &mut UartRx<'_, peripherals::USART6, Blocking>) {
    while rx.nb_read().is_ok() {}
}

fn send_cmd(tx: &mut UartTx<'_, peripherals::USART6, Blocking>, cmd: &[u8]) {
    let _ = tx.blocking_write(cmd);
}

fn read_response(
    rx: &mut UartRx<'_, peripherals::USART6, Blocking>,
    delay: &mut Delay,
    out: &mut [u8],
    idle_timeout_ms: u32,
) -> usize {
    let mut len = 0usize;
    let mut idle_ms = 0u32;

    // 轮询读取：每当收到了新字节就重置空闲计时，空闲超时后结束本次响应采集。
    while len < out.len() && idle_ms < idle_timeout_ms {
        if let Ok(byte) = rx.nb_read() {
            out[len] = byte;
            len += 1;
            idle_ms = 0;
        } else {
            idle_ms += POLL_INTERVAL_MS;
            delay.delay_ms(POLL_INTERVAL_MS);
        }
    }

    len
}

fn run_at_command(
    tx: &mut UartTx<'_, peripherals::USART6, Blocking>,
    rx: &mut UartRx<'_, peripherals::USART6, Blocking>,
    delay: &mut Delay,
    cmd: &[u8],
    expect: &[u8],
) -> bool {
    send_cmd(tx, cmd);

    let mut response = [0u8; RESPONSE_BUF_SIZE];
    let size = read_response(rx, delay, &mut response, RESPONSE_IDLE_TIMEOUT_MS);
    let payload = &response[..size];

    if let Ok(text) = core::str::from_utf8(payload) {
        println!("wifi<= {}", text);
    } else {
        println!("wifi<= <non-utf8: {} bytes>", size);
    }

    contains_token(payload, expect)
}

#[hal::entry]
fn main() -> ! {
    hal::debug::SDIPrint::enable();
    let p = hal::init(Default::default());

    // LED1(PE11) = 通信通过指示；LED2(PE12) = 通信失败指示（低电平点亮）
    let mut led_ok = Output::new(p.PE11, Level::High, Default::default());
    let mut led_fail = Output::new(p.PE12, Level::High, Default::default());
    let mut delay = Delay;

    let mut cfg = usart::Config::default();
    cfg.baudrate = WIFI_BAUDRATE;

    // openCH 赤菟 WiFi 接口：PC0=UART6_TX, PC1=UART6_RX，对应 REMAP=0。
    let wifi = Uart::new_blocking::<0>(p.USART6, p.PC1, p.PC0, cfg).unwrap();
    let (mut wifi_tx, mut wifi_rx) = wifi.split();

    delay.delay_ms(500);
    drain_rx(&mut wifi_rx);

    let at_ok = run_at_command(
        &mut wifi_tx,
        &mut wifi_rx,
        &mut delay,
        b"AT\r\n",
        b"OK",
    );

    let gmr_ok = run_at_command(
        &mut wifi_tx,
        &mut wifi_rx,
        &mut delay,
        b"AT+GMR\r\n",
        b"OK",
    );

    if at_ok && gmr_ok {
        println!("wifi init check: pass");
        led_fail.set_high();
        loop {
            led_ok.toggle();
            delay.delay_ms(150);
        }
    }

    println!("wifi init check: fail");
    led_ok.set_high();
    loop {
        led_fail.set_low();
        delay.delay_ms(120);
        led_fail.set_high();
        delay.delay_ms(900);
    }
}
