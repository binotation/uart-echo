#![no_std]
#![no_main]

use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
                     // use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
use cortex_m_rt::entry;
use stm32l4::stm32l4x2::{self, interrupt};

static mut USART1_PERIPHERAL: Option<stm32l4x2::USART1> = None;

#[interrupt]
fn USART1() {
    // SAFETY: race condition where USART1_PERIPHERAL can be accessed before being set but this is impossible.
    if let Some(usart1) = unsafe { USART1_PERIPHERAL.as_mut() } {
        if usart1.isr.read().rxne().bit_is_set() {
            let received_byte = usart1.rdr.read().rdr().bits(); // Reading RDR clears RXNE
            while usart1.isr.read().txe().bit_is_clear() {} // Poll TXE, should already be set
            usart1.tdr.write(|w| w.tdr().bits(received_byte - 32));
        }
    }
}

#[entry]
fn main() -> ! {
    // Device defaults to 4MHz clock

    let dp = stm32l4x2::Peripherals::take().unwrap();

    // Enable peripheral clocks - GPIOA, USART1
    dp.RCC.ahb2enr.write(|w| w.gpioaen().set_bit());
    dp.RCC.apb2enr.write(|w| w.usart1en().set_bit());

    // Configure A9 (TX), A10 (RX) as alternate function 7
    dp.GPIOA
        .moder
        .write(|w| w.moder9().alternate().moder10().alternate());
    dp.GPIOA
        .ospeedr
        .write(|w| w.ospeedr9().high_speed().ospeedr10().high_speed());
    dp.GPIOA.afrh.write(|w| w.afrh9().af7().afrh10().af7());

    // Configure baud rate 9600
    dp.USART1.brr.write(|w| w.brr().bits(417)); // 4Mhz / 9600 approx. 417

    // Enable USART, transmitter, receiver and RXNE interrupt
    dp.USART1.cr1.write(|w| {
        w.te()
            .enabled()
            .re()
            .enabled()
            .ue()
            .enabled()
            .rxneie()
            .enabled()
    });

    unsafe {
        // Unmask NVIC USART1 global interrupt
        cortex_m::peripheral::NVIC::unmask(stm32l4x2::Interrupt::USART1);
        USART1_PERIPHERAL = Some(dp.USART1);
    }

    #[allow(clippy::empty_loop)]
    loop {}
}
