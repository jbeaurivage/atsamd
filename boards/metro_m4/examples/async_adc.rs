#![no_std]
#![no_main]

use metro_m4 as bsp;

use bsp::hal;
use bsp::pac;

#[cfg(not(feature = "use_semihosting"))]
use panic_halt as _;
#[cfg(feature = "use_semihosting")]
use panic_semihosting as _;

use bsp::Pins;
use pac::{CorePeripherals, Peripherals};

use hal::{
    adc::{Accumulation, Adc, Adc0, Config, Prescaler, Resolution},
    clock::v2::{clock_system_at_reset, pclk::Pclk},
};

atsamd_hal::bind_multiple_interrupts!(struct Irqs {
    ADC0: [ADC0_RESRDY, ADC0_OTHER] => atsamd_hal::adc::InterruptHandler<Adc0>;
});

#[embassy_executor::main]
async fn main(_s: embassy_executor::Spawner) -> ! {
    let mut peripherals = Peripherals::take().unwrap();
    let _core = CorePeripherals::take().unwrap();

    let pins = Pins::new(peripherals.port);

    let (mut buses, clocks, tokens) = clock_system_at_reset(
        peripherals.oscctrl,
        peripherals.osc32kctrl,
        peripherals.gclk,
        peripherals.mclk,
        &mut peripherals.nvmctrl,
    );

    // Enable the ADC0 ABP clock...
    let apb_adc0 = buses.apb.enable(tokens.apbs.adc0);
    // ...and enable the ADC0 PCLK. Both of these are required for the
    // ADC to run.
    let (pclk_adc0, _gclk0) = Pclk::enable(tokens.pclks.adc0, clocks.gclk0);

    let adc0_settings = Config::new()
        .clock_cycles_per_sample(5)
        .clock_divider(Prescaler::Div32)
        .sample_resolution(Resolution::_12bit)
        .accumulation_method(Accumulation::Single);

    let mut adc = Adc::new(peripherals.adc0, adc0_settings, apb_adc0, &pclk_adc0)
        .unwrap()
        .into_future(Irqs);
    let mut adc_pin = pins.a0.into_alternate();

    loop {
        let _res = adc.read(&mut adc_pin).await.unwrap();
        #[cfg(feature = "use_semihosting")]
        cortex_m_semihosting::hprintln!("Result: {:?}", _res).unwrap();
    }
}
