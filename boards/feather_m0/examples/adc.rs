#![no_std]
#![no_main]

use feather_m0 as bsp;

use bsp::hal;
use bsp::pac;

#[cfg(not(feature = "use_semihosting"))]
use panic_halt as _;
#[cfg(feature = "use_semihosting")]
use panic_semihosting as _;

use bsp::entry;
use bsp::Pins;
use pac::{CorePeripherals, Peripherals};

use hal::{
    adc::{Accumulation, Adc, Config, Prescaler, Resolution},
    clock::GenericClockController,
};

#[entry]
fn main() -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let _core = CorePeripherals::take().unwrap();

    let pins = Pins::new(peripherals.port);

    // SAMD21 targets currently don't support clock::v2
    let mut clocks = GenericClockController::with_external_32kosc(
        peripherals.gclk,
        &mut peripherals.pm,
        &mut peripherals.sysctrl,
        &mut peripherals.nvmctrl,
    );
    let gclk0 = clocks.gclk0();
    let adc_clock = clocks.adc(&gclk0).unwrap();

    let adc_settings = Config::new()
        .clock_cycles_per_sample(5)
        .clock_divider(Prescaler::Div128)
        .sample_resolution(Resolution::_12bit)
        .accumulation_method(Accumulation::Single);

    let mut adc = Adc::new(
        peripherals.adc,
        adc_settings,
        &mut peripherals.pm,
        &adc_clock,
    )
    .unwrap();
    let mut adc_pin = pins.a0.into_alternate();

    loop {
        let _res = adc.read_blocking(&mut adc_pin).unwrap();
        #[cfg(feature = "use_semihosting")]
        cortex_m_semihosting::hprintln!("Result: {:?}", _res).unwrap();
    }
}
