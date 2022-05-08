use crate::{
    async_hal::interrupt,
    dmac::Channels,
    pac::{DMAC, PM},
};
use embassy::interrupt::InterruptExt;

// BitIter shamelessly stolen from embassy:
// https://github.com/embassy-rs/embassy/blob/3d1501c02038e5fe6f6d3b72bd18bd7a52595a77/embassy-stm32/src/exti.rs#L67
struct BitIter(u32);

impl Iterator for BitIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.trailing_zeros() {
            32 => None,
            b => {
                self.0 &= !(1 << b);
                Some(b)
            }
        }
    }
}

#[cfg(any(feature = "samd11", feature = "samd21"))]
mod thumbv6m {
    use super::*;
    use crate::dmac::waker::WAKERS;

    /// Initialized DMA Controller
    pub struct DmaController {
        pub(in super::super) dmac: DMAC,
        pub(in super::super) irq: crate::async_hal::interrupt::DMAC,
    }

    impl DmaController {
        /// Perform additional async-specific setup to turn a [`DMAC`] into a
        /// [`DmaController`]
        pub(in super::super) fn new_async(dmac: DMAC) -> Self {
            #[cfg(any(feature = "samd11", feature = "samd21"))]
            {
                let irq = interrupt::take!(DMAC);
                irq.set_handler(on_interrupt);
                irq.enable();

                Self { dmac, irq }
            }
        }
    }

    unsafe fn on_interrupt(_: *mut ()) {
        // SAFETY: Here we can't go through the `with_chid` method to safely access
        // the different channel interrupt flags. Instead, we read the ID in a short critical
        // section, and make sure to RESET the CHID field to whatever it was before
        // this function ran.
        let dmac = crate::pac::Peripherals::steal().DMAC;

        cortex_m::interrupt::free(|_| {
            let intpend = &dmac.intpend;
            let old_id = intpend.read().id().bits();
            let pending_interrupts = BitIter(dmac.intstatus.read().bits());

            // TODO notify task that there is an error?

            // Iterate over channels and check their interrupt status
            for pend_channel in pending_interrupts {
                intpend.modify(|_, w| w.id().bits(pend_channel as u8));

                let wake = if intpend.read().tcmpl().bit_is_set() {
                    // Transfer complete
                    intpend.modify(|_, w| w.tcmpl().set_bit());
                    true
                } else if intpend.read().terr().bit_is_set() {
                    // Transfer error
                    intpend.modify(|_, w| w.terr().set_bit());
                    true
                } else {
                    false
                };

                if wake {
                    WAKERS[pend_channel as usize].wake();
                }
            }

            // Reset the INTPEND.ID register
            intpend.write(|w| w.id().bits(old_id));
        });
    }
}

#[cfg(any(feature = "samd11", feature = "samd21"))]
pub use thumbv6m::*;

#[cfg(feature = "min-samd51g")]
mod thumbv7em {
    use super::*;

    /// Initialized DMA Controller
    pub struct DmaController {
        pub(in super::super) dmac: DMAC,
        pub(in super::super) irq_0: crate::async_hal::interrupt::DMAC_0,
        pub(in super::super) irq_1: crate::async_hal::interrupt::DMAC_1,
        pub(in super::super) irq_2: crate::async_hal::interrupt::DMAC_2,
        pub(in super::super) irq_3: crate::async_hal::interrupt::DMAC_3,
        pub(in super::super) irq_other: crate::async_hal::interrupt::DMAC_OTHER,
    }

    impl DmaController {
        /// Perform additional async-specific setup to turn a [`DMAC`] into a
        /// [`DmaController`]
        pub(in super::super) fn new_async(dmac: DMAC) -> Self {
            #[cfg(any(feature = "samd11", feature = "samd21"))]
            {
                let irq = interrupt::take!(DMAC);
                irq.set_handler(on_interrupt);
                irq.enable();

                Self { dmac, irq }
            }

            #[cfg(feature = "min-samd51g")]
            {
                Self {
                    dmac,
                    irq_0: interrupt::take!(DMAC_0),
                    irq_1: interrupt::take!(DMAC_1),
                    irq_2: interrupt::take!(DMAC_2),
                    irq_3: interrupt::take!(DMAC_3),
                    irq_other: interrupt::take!(DMAC_OTHER),
                }
            }
        }
    }

    // TODO do something in the interrupt handler
    // TODO wake corresponding waker in async_api::WAKERS
    unsafe fn on_dmac_0(_: *mut ()) {}
    unsafe fn on_dmac_1(_: *mut ()) {}
    unsafe fn on_dmac_2(_: *mut ()) {}
    unsafe fn on_dmac_3(_: *mut ()) {}
    unsafe fn on_dmac_other(_: *mut ()) {}
}

#[cfg(feature = "min-samd51g")]
pub use thumbv7em::*;

impl DmaController {
    /// Release the DMAC and return the register block.
    ///
    /// **Note**: The [`Channels`] struct is consumed by this method. This means
    /// that any [`Channel`] obtained by [`split`](DmaController::split) must be
    /// moved back into the [`Channels`] struct before being able to pass it
    /// into [`free`](DmaController::free).
    #[inline]
    pub fn free(mut self, _channels: Channels, _pm: &mut PM) -> DMAC {
        self.dmac.ctrl.modify(|_, w| w.dmaenable().clear_bit());

        Self::swreset(&mut self.dmac);

        #[cfg(any(feature = "samd11", feature = "samd21"))]
        {
            // Disable the DMAC clocking
            _pm.apbbmask.modify(|_, w| w.dmac_().clear_bit());
            _pm.ahbmask.modify(|_, w| w.dmac_().clear_bit());
        }

        self.irq.remove_handler();
        self.irq.disable();

        // Release the DMAC
        self.dmac
    }
}
