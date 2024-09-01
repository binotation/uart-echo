//! Use USART2.

#![no_std]
#![no_main]

use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
                     // use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
use cortex_m_rt::entry;
use heapless::spsc::Queue;
use stm32u5::stm32u575::{interrupt, Interrupt, Peripherals, USART2};

static mut USART2_PERIPHERAL: Option<USART2> = None;
static mut BUFFER: Option<Queue<u16, 8>> = None;

#[interrupt]
fn USART2() {
    // SAFETY: race condition where USART2_PERIPHERAL can be accessed before being set
    let usart2 = unsafe { USART2_PERIPHERAL.as_mut() }.unwrap();
    let buffer = unsafe { BUFFER.as_mut() }.unwrap();

    if usart2.isr_disabled().read().txfnf().bit_is_set() {
        match buffer.dequeue() {
            // Write dequeued byte
            Some(byte) => {
                usart2.tdr().write(|w| unsafe { w.tdr().bits(byte) });
                if buffer.is_empty() {
                    usart2.cr1_disabled().modify(|_, w| w.txfnfie().clear_bit());
                }
            }
            // Buffer is empty, disable TXE interrupt
            None => usart2.cr1_disabled().modify(|_, w| w.txfnfie().clear_bit()),
        }
    }
    if usart2.isr_disabled().read().rxfne().bit_is_set() {
        // Read data, this clears RXNE
        let received_byte = usart2.rdr().read().rdr().bits();

        // Queue byte - 32, do nothing if queue is full
        if buffer.enqueue(received_byte - 32).is_ok() {
            // Enable TXE interrupt as buffer is now non-empty
            usart2.cr1_disabled().modify(|_, w| w.txfnfie().set_bit());
        }
    }
    if usart2.isr_disabled().read().ore().bit_is_set() {
        usart2.icr().write(|w| w.orecf().set_bit());
    }
}

#[entry]
fn main() -> ! {
    // Device defaults to 4MHz clock

    let dp = Peripherals::take().unwrap();

    // Enable peripheral clocks - GPIOA, USART2
    dp.RCC.ahb2enr1().write(|w| w.gpioaen().enabled());
    dp.RCC.apb1enr1().write(|w| w.usart2en().enabled());

    // Configure A2 (TX), A3 (RX) as alternate function 7
    dp.GPIOA
        .moder()
        .write(|w| w.mode2().alternate().mode3().alternate());
    dp.GPIOA
        .ospeedr()
        .write(|w| w.ospeed2().high_speed().ospeed3().high_speed());
    dp.GPIOA.afrl().write(|w| w.afsel2().af7().afsel3().af7());

    // Configure baud rate 9600
    dp.USART2.brr().write(|w| unsafe { w.bits(417) }); // 4Mhz / 9600 approx. 417

    // Enable USART, transmitter, receiver and RXNE interrupt
    dp.USART2.cr1_disabled().write(|w| {
        w.re()
            .set_bit()
            .te()
            .set_bit()
            .ue()
            .set_bit()
            .rxfneie()
            .set_bit()
    });

    unsafe {
        BUFFER = Some(Queue::default());
        // Unmask NVIC USART2 global interrupt
        cortex_m::peripheral::NVIC::unmask(Interrupt::USART2);
        USART2_PERIPHERAL = Some(dp.USART2);
    }

    #[allow(clippy::empty_loop)]
    loop {}
}
