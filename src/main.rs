#![no_std]
#![no_main]

use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
                     // use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
use cortex_m_rt::entry;
use heapless::spsc::Queue;
use stm32l4::stm32l4x2::{interrupt, Interrupt, Peripherals, USART1};

static mut USART1_PERIPHERAL: Option<USART1> = None;
static mut BUFFER: Option<Queue<u16, 8>> = None;

#[interrupt]
fn USART1() {
    // SAFETY: race condition where USART1_PERIPHERAL can be accessed before being set
    let usart1 = unsafe { USART1_PERIPHERAL.as_mut() }.unwrap();
    let buffer = unsafe { BUFFER.as_mut() }.unwrap();

    if usart1.isr.read().txe().bit_is_set() {
        match buffer.dequeue() {
            // Write dequeued byte
            Some(byte) => {
                usart1.tdr.write(|w| w.tdr().bits(byte));
                if buffer.is_empty() {
                    usart1.cr1.modify(|_, w| w.txeie().disabled());
                }
            }
            // Buffer is empty, disable TXE interrupt
            None => usart1.cr1.modify(|_, w| w.txeie().disabled()),
        }
    }
    if usart1.isr.read().rxne().bit_is_set() {
        // Read data, this clears RXNE
        let received_byte = usart1.rdr.read().rdr().bits();

        // Queue byte - 32, do nothing if queue is full
        if buffer.enqueue(received_byte - 32).is_ok() {
            // Enable TXE interrupt as buffer is now non-empty
            usart1.cr1.modify(|_, w| w.txeie().enabled());
        }
    }
    if usart1.isr.read().ore().bit_is_set() {
        usart1.icr.write(|w| w.orecf().set_bit());
    }
}

#[entry]
fn main() -> ! {
    // Device defaults to 4MHz clock

    let dp = Peripherals::take().unwrap();

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
        w.re()
            .enabled()
            .te()
            .enabled()
            .ue()
            .enabled()
            .rxneie()
            .enabled()
    });

    unsafe {
        BUFFER = Some(Queue::default());
        // Unmask NVIC USART1 global interrupt
        cortex_m::peripheral::NVIC::unmask(Interrupt::USART1);
        USART1_PERIPHERAL = Some(dp.USART1);
    }

    #[allow(clippy::empty_loop)]
    loop {}
}
