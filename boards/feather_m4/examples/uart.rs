//! This example showcases the uart::v2 module.

#![no_std]
#![no_main]

use cortex_m::asm;
use panic_halt as _;

use bsp::hal;
use bsp::pac;
use feather_m4 as bsp;

use bsp::{entry, periph_alias, pin_alias};
use hal::clock::GenericClockController;
use hal::dmac::{DmaController, PriorityLevel};
use hal::ehal_nb::serial::{Read, Write};
use hal::fugit::RateExtU32;
use hal::nb;

use pac::Peripherals;

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let mut clocks = GenericClockController::with_internal_32kosc(
        peripherals.GCLK,
        &mut peripherals.MCLK,
        &mut peripherals.OSC32KCTRL,
        &mut peripherals.OSCCTRL,
        &mut peripherals.NVMCTRL,
    );

    let mut mclk = peripherals.MCLK;
    let dmac = peripherals.DMAC;
    let pins = bsp::Pins::new(peripherals.PORT);

    // Take RX and TX pins
    let uart_rx = pin_alias!(pins.uart_rx);
    let uart_tx = pin_alias!(pins.uart_tx);

    // Setup DMA channels for later use
    let mut dmac = DmaController::init(dmac, &mut peripherals.PM);
    let channels = dmac.split();

    let chan0 = channels.0.init(PriorityLevel::LVL0);
    let chan1 = channels.1.init(PriorityLevel::LVL0);

    let uart_sercom = periph_alias!(peripherals.uart_sercom);

    // Setup UART peripheral
    let uart = bsp::uart(
        &mut clocks,
        9600.Hz(),
        uart_sercom,
        &mut mclk,
        uart_rx,
        uart_tx,
    );

    // Split uart in rx + tx halves
    let (mut rx, mut tx) = uart.split();

    // Get a 50 byte buffer to store data to send/receive
    const LENGTH: usize = 50;
    let rx_buffer: &'static mut [u8; LENGTH] =
        cortex_m::singleton!(: [u8; LENGTH] = [0x00; LENGTH]).unwrap();
    let tx_buffer: &'static mut [u8; LENGTH] =
        cortex_m::singleton!(: [u8; LENGTH] = [0x00; LENGTH]).unwrap();

    // For fun, store numbers from 0 to 49 in buffer
    for (i, c) in tx_buffer.iter_mut().enumerate() {
        *c = i as u8;
    }

    // Send data in a blocking way
    for c in tx_buffer.iter() {
        nb::block!(tx.write(*c)).unwrap();
    }

    // We'll now receive data in a blocking way
    rx.flush_rx_buffer();
    for c in rx_buffer.iter_mut() {
        *c = nb::block!(rx.read()).unwrap();
    }

    // Finally, we'll receive AND send data at the same time with DMA

    // Setup a DMA transfer to send our data asynchronously.
    // We'll set the waker to be a no-op
    let tx_dma = tx.send_with_dma(tx_buffer, chan0);

    // Setup a DMA transfer to receive our data asynchronously.
    // Again, we'll set the waker to be a no-op
    let rx_dma = rx.receive_with_dma(rx_buffer, chan1);

    // Wait for transmit DMA transfer to complete
    let (_chan0, _tx_buffer, _tx) = tx_dma.wait();

    // Wait for receive DMA transfer to complete
    let (_chan1, _rx_buffer, _rx) = rx_dma.wait();

    loop {
        // Go to sleep
        asm::wfi();
    }
}
