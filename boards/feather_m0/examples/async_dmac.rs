//! This example shows a safe API to
//! execute a memory-to-memory DMA transfer

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

#[rtic::app(device = bsp::pac, dispatchers = [I2S])]
mod app {
    use bsp::{hal, pac, pin_alias};
    use feather_m0 as bsp;
    use hal::{
        clock::{enable_internal_32kosc, ClockGenId, ClockSource, GenericClockController},
        dmac::{
            Ch0, Channel, DmaController, PriorityLevel, ReadyFuture, Transfer, TriggerAction,
            TriggerSource,
        },
        rtc::{Count32Mode, Rtc},
    };

    #[monotonic(binds = RTC, default = true)]
    type Monotonic = Rtc<Count32Mode>;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        channel: Channel<Ch0, ReadyFuture>,
        source: &'static mut [u8; 50],
        dest: &'static mut [u8; 50],
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        let mut peripherals = cx.device;
        let _core = cx.core;

        let mut clocks = GenericClockController::with_external_32kosc(
            peripherals.GCLK,
            &mut peripherals.PM,
            &mut peripherals.SYSCTRL,
            &mut peripherals.NVMCTRL,
        );

        enable_internal_32kosc(&mut peripherals.SYSCTRL);
        let timer_clock = clocks
            .configure_gclk_divider_and_source(ClockGenId::GCLK2, 1, ClockSource::OSC32K, false)
            .unwrap();
        clocks.configure_standby(ClockGenId::GCLK2, true);

        // Setup RTC monotonic
        let rtc_clock = clocks.rtc(&timer_clock).unwrap();
        let rtc = Rtc::count32_mode(peripherals.RTC, rtc_clock.freq(), &mut peripherals.PM);

        // Initialize buffers
        const LENGTH: usize = 50;
        let source: &'static mut [u8; LENGTH] =
            cortex_m::singleton!(: [u8; LENGTH] = [0xff; LENGTH]).unwrap();
        let dest: &'static mut [u8; LENGTH] =
            cortex_m::singleton!(: [u8; LENGTH] = [0x00; LENGTH]).unwrap();

        // Initialize DMA Controller
        let dmac = DmaController::init(peripherals.DMAC, &mut peripherals.PM);
        // Get handle to IRQ
        let dmac_irq = cortex_m_interrupt::take_nvic_interrupt!(pac::Interrupt::DMAC, 2);
        // Turn dmac into an async controller
        let mut dmac = dmac.into_future(dmac_irq);
        // Get individual handles to DMA channels
        let channels = dmac.split();

        // Initialize DMA Channel 0
        let channel = channels.0.init(PriorityLevel::LVL0);

        async_task::spawn().ok();

        (
            Shared {},
            Local {
                channel,
                source,
                dest,
            },
            init::Monotonics(rtc),
        )
    }

    #[task(local = [channel, source, dest])]
    async fn async_task(cx: async_task::Context) {
        let async_task::LocalResources {
            channel,
            source,
            dest,
        } = cx.local;

        defmt::info!(
            "Launching a DMA transfer.\n\tSource: {}\n\tDestination: {}",
            &source,
            &dest
        );
        Transfer::transfer_future(
            channel,
            source,
            dest,
            TriggerSource::DISABLE,
            TriggerAction::BLOCK,
        )
        .await
        .unwrap();

        defmt::info!(
            "Launching a DMA transfer.\n\tSource: {}\n\tDestination: {}",
            &source,
            &dest
        );

        loop {
            cortex_m::asm::wfi();
        }
    }
}
