//! # Abstractions to setup and use the DMA controller
//!
//! # Initializing
//!
//! The DMAC should be initialized using the
//! [`DmaController::init`] method. It will consume the
//! DMAC object generated by the PAC. By default, all four priority levels
//! will be enabled, but can be selectively enabled/disabled through the
//! [`DmaController::enable_levels`] ansd [`DmaController::disable_levels`]
//! methods.
//!
//! # Splitting Channels
//!
//! Using the [`DmaController::split`] method will return
//! a struct containing handles to individual channels.
//!
//! # Releasing the DMAC
//!
//! Using the [`free`](DmaController::free) method will
//! deinitialize the DMAC and return the underlying PAC object.

use core::marker::PhantomData;

use modular_bitfield::prelude::*;
use seq_macro::seq;

#[cfg(feature = "thumbv6")]
pub use crate::pac::dmac::chctrlb::{
    LVLSELECT_A as PriorityLevel, TRIGACTSELECT_A as TriggerAction,
    TRIGSRCSELECT_A as TriggerSource,
};

#[cfg(feature = "thumbv7")]
pub use crate::pac::dmac::channel::{
    chctrla::{
        BURSTLENSELECT_A as BurstLength, THRESHOLDSELECT_A as FifoThreshold,
        TRIGACTSELECT_A as TriggerAction, TRIGSRCSELECT_A as TriggerSource,
    },
    chprilvl::PRILVLSELECT_A as PriorityLevel,
};

#[cfg(all(feature = "async", feature = "thumbv6"))]
type Irq = crate::async_hal::interrupts::DMAC;

/// On thumbv7 targets, we can only check that one interrupt is correctly bound,
/// lest we dive into typelevel insanity once more. We just have to trust the
/// user has bound all relevant interrupts sources.
#[cfg(all(feature = "async", feature = "thumbv7"))]
type Irq = crate::async_hal::interrupts::DMAC_0;

use super::{
    channel::{Channel, Uninitialized},
    DESCRIPTOR_SECTION, WRITEBACK,
};
use crate::{
    pac::{DMAC, PM},
    typelevel::NoneT,
};

/// Trait representing a DMA channel ID
pub trait ChId {
    const U8: u8;
    const USIZE: usize;
}

macro_rules! define_channels_struct {
    ($num_channels:literal) => {
        seq!(N in 0..$num_channels {
            #(
                /// Type alias for a channel number
                pub struct Ch~N;

                impl ChId for Ch~N {
                    const U8: u8 = N;
                    const USIZE: usize = N;
                }
            )*

            /// Struct generating individual handles to each DMA channel
            pub struct Channels(
                #(
                    pub Channel<Ch~N, Uninitialized>,
                )*
            );
        });
    };
}

with_num_channels!(define_channels_struct);

#[cfg(feature = "async")]
macro_rules! define_channels_struct_future {
    ($num_channels:literal) => {
        seq!(N in 0..$num_channels {
            /// Struct generating individual handles to each DMA channel for `async` operation
            pub struct FutureChannels(
                #(
                    pub Channel<Ch~N, super::channel::UninitializedFuture>,
                )*
            );
        });
    };
}

#[cfg(feature = "async")]
with_num_channels!(define_channels_struct_future);

/// Initialized DMA Controller
pub struct DmaController<I = NoneT> {
    dmac: DMAC,
    _irqs: PhantomData<I>,
}

/// Mask representing which priority levels should be enabled/disabled
#[bitfield]
#[repr(u16)]
pub struct PriorityLevelMask {
    #[skip]
    _reserved: B8,
    /// Level 0
    #[allow(dead_code)]
    level0: bool,
    /// Level 1
    #[allow(dead_code)]
    level1: bool,
    /// Level 2
    #[allow(dead_code)]
    level2: bool,
    /// Level 3
    #[allow(dead_code)]
    level3: bool,
    #[skip]
    _reserved: B4,
}

/// Mask representing which priority levels should be configured as round-robin
#[bitfield]
#[repr(u32)]
pub struct RoundRobinMask {
    #[skip]
    _reserved: B7,
    /// Level 0
    #[allow(dead_code)]
    level0: bool,
    #[skip]
    _reserved: B7,
    /// Level 1
    #[allow(dead_code)]
    level1: bool,
    #[skip]
    _reserved: B7,
    /// Level 2
    #[allow(dead_code)]
    level2: bool,
    #[skip]
    _reserved: B7,
    /// Level 3
    #[allow(dead_code)]
    level3: bool,
}

impl<T> DmaController<T> {
    /// Enable multiple priority levels simultaneously
    #[inline]
    pub fn enable_levels(&mut self, mask: PriorityLevelMask) {
        // SAFETY This is safe because the use of bitfields ensures that only the
        // LVLENx bits are written to. The fact that we are given a mask means we need
        // to do the bit-level setting ourselves.
        let mask: u16 = mask.into();
        unsafe {
            self.dmac.ctrl.modify(|r, w| w.bits(r.bits() | mask));
        }
    }

    /// Disable multiple priority levels simultaneously
    #[inline]
    pub fn disable_levels(&mut self, mask: PriorityLevelMask) {
        // SAFETY This is safe because the use of bitfields ensures that only the
        // LVLENx bits are written to. The fact that we are given a mask means we need
        // to do the bit-level clearing ourselves.
        let mask: u16 = mask.into();
        unsafe {
            self.dmac.ctrl.modify(|r, w| w.bits(r.bits() & !mask));
        }
    }

    /// Enable round-robin arbitration for multiple priority levels
    /// simultaneously
    #[inline]
    pub fn round_robin_arbitration(&mut self, mask: RoundRobinMask) {
        // SAFETY This is safe because the use of bitfields ensures that only the
        // RRLVLENx bits are written to. The fact that we are given a mask means we need
        // to do the bit-level setting ourselves.
        let mask: u32 = mask.into();
        unsafe {
            self.dmac.prictrl0.modify(|r, w| w.bits(r.bits() | mask));
        }
    }

    /// Disable round-robin arbitration (ie, enable static priorities) for
    /// multiple priority levels simultaneously
    #[inline]
    pub fn static_arbitration(&mut self, mask: RoundRobinMask) {
        // SAFETY This is safe because the use of bitfields ensures that only the
        // RRLVLENx bits are written to. The fact that we are given a mask means we need
        // to do the bit-level clearing ourselves.
        let mask: u32 = mask.into();
        unsafe {
            self.dmac.prictrl0.modify(|r, w| w.bits(r.bits() & !mask));
        }
    }

    /// Use the [`DmaController`] in async mode. You are required to provide the
    /// struct created by the
    /// [`bind_interrupts`](crate::bind_interrupts) macro to prove
    /// that the interrupt sources have been correctly configured. This function
    /// will automatically enable the relevant NVIC interrupt sources. However,
    /// you are required to configure the desired interrupt priorities prior to
    /// calling this method. Consult [`crate::async_hal::interrupts`]
    /// module-level documentation for more information.
    ///
    /// # Note for SAMx5x users
    ///
    /// The DMAC on SAMD51/SAMD53/SAME53/SAME54 has 5 NVIC interrupt sources:
    /// * DMAC_0 (channel 0)
    /// * DMAC_1 (channel 1)
    /// * DMAC_2 (channel 2)
    /// * DMAC_3 (channel 3)
    /// * DMAC_OTHER (all other channels).
    ///
    /// You **must** bind all 5 sources using
    /// [`bind_interrupts`](crate::bind_interrupts).
    #[cfg(feature = "async")]
    #[inline]
    pub fn into_future<I>(self, _interrupts: I) -> DmaController<I>
    where
        I: crate::async_hal::interrupts::Binding<Irq, super::async_api::InterruptHandler>,
    {
        use crate::async_hal::interrupts::Interrupt;

        #[cfg(feature = "thumbv6")]
        {
            use crate::async_hal::interrupts::DMAC;
            DMAC::unpend();
            unsafe { DMAC::enable() };
        }

        #[cfg(feature = "thumbv7")]
        {
            use crate::async_hal::interrupts::{DMAC_0, DMAC_1, DMAC_2, DMAC_3, DMAC_OTHER};
            DMAC_0::unpend();
            DMAC_1::unpend();
            DMAC_2::unpend();
            DMAC_3::unpend();
            DMAC_OTHER::unpend();
            unsafe {
                DMAC_0::enable();
                DMAC_1::enable();
                DMAC_2::enable();
                DMAC_3::enable();
                DMAC_OTHER::enable();
            }
        }

        DmaController {
            dmac: self.dmac,
            _irqs: PhantomData,
        }
    }

    /// Issue a software reset to the DMAC and wait for reset to complete
    #[inline]
    fn swreset(dmac: &mut DMAC) {
        dmac.ctrl.modify(|_, w| w.swrst().set_bit());
        while dmac.ctrl.read().swrst().bit_is_set() {}
    }
}

impl DmaController {
    /// Initialize the DMAC and return a DmaController object useable by
    /// [`Transfer`](super::transfer::Transfer)'s. By default, all
    /// priority levels are enabled unless subsequently disabled using the
    /// `level_x_enabled` methods.
    #[inline]
    pub fn init(mut dmac: DMAC, _pm: &mut PM) -> Self {
        // ----- Initialize clocking ----- //
        #[cfg(feature = "thumbv6")]
        {
            // Enable clocking
            _pm.ahbmask.modify(|_, w| w.dmac_().set_bit());
            _pm.apbbmask.modify(|_, w| w.dmac_().set_bit());
        }

        Self::swreset(&mut dmac);

        // SAFETY this is safe because we write a whole u32 to 32-bit registers,
        // and the descriptor array addesses will never change since they are static.
        // We just need to ensure the writeback and descriptor_section addresses
        // are valid.
        unsafe {
            dmac.baseaddr
                .write(|w| w.baseaddr().bits(DESCRIPTOR_SECTION.as_ptr() as u32));
            dmac.wrbaddr
                .write(|w| w.wrbaddr().bits(WRITEBACK.as_ptr() as u32));
        }

        // ----- Select priority levels ----- //
        dmac.ctrl.modify(|_, w| {
            w.lvlen3().set_bit();
            w.lvlen2().set_bit();
            w.lvlen1().set_bit();
            w.lvlen0().set_bit()
        });

        // Enable DMA controller
        dmac.ctrl.modify(|_, w| w.dmaenable().set_bit());
        Self {
            dmac,
            _irqs: PhantomData,
        }
    }

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

        #[cfg(feature = "thumbv6")]
        {
            // Disable the DMAC clocking
            _pm.apbbmask.modify(|_, w| w.dmac_().clear_bit());
            _pm.ahbmask.modify(|_, w| w.dmac_().clear_bit());
        }

        // Release the DMAC
        self.dmac
    }
}

#[cfg(feature = "async")]
impl<I> DmaController<I>
where
    I: crate::async_hal::interrupts::Binding<Irq, super::async_api::InterruptHandler>,
{
    /// Release the DMAC and return the register block.
    ///
    /// **Note**: The [`Channels`] struct is consumed by this method. This means
    /// that any [`Channel`] obtained by [`split`](DmaController::split) must be
    /// moved back into the [`Channels`] struct before being able to pass it
    /// into [`free`](DmaController::free).
    #[inline]
    pub fn free(mut self, _channels: FutureChannels, _pm: &mut PM) -> DMAC {
        self.dmac.ctrl.modify(|_, w| w.dmaenable().clear_bit());

        Self::swreset(&mut self.dmac);

        #[cfg(any(feature = "samd11", feature = "samd21"))]
        {
            // Disable the DMAC clocking
            _pm.apbbmask.modify(|_, w| w.dmac_().clear_bit());
            _pm.ahbmask.modify(|_, w| w.dmac_().clear_bit());
        }

        // Release the DMAC
        self.dmac
    }
}

macro_rules! define_split {
    ($num_channels:literal) => {
        seq!(N in 0..$num_channels {
            /// Split the DMAC into individual channels
            #[inline]
            pub fn split(&mut self) -> Channels {
                Channels(
                    #(
                        crate::dmac::channel::new_chan(core::marker::PhantomData),
                    )*
                )
            }
        });
    };
}

impl DmaController {
    with_num_channels!(define_split);
}

#[cfg(feature = "async")]
macro_rules! define_split_future {
    ($num_channels:literal) => {
        seq!(N in 0..$num_channels {
            /// Split the DMAC into individual channels
            #[inline]
            pub fn split(&mut self) -> FutureChannels {
                FutureChannels(
                    #(
                        crate::dmac::channel::new_chan_future(core::marker::PhantomData),
                    )*
                )
            }
        });
    };
}

#[cfg(feature = "async")]
impl<I> DmaController<I>
where
    I: crate::async_hal::interrupts::Binding<Irq, super::async_api::InterruptHandler>,
{
    with_num_channels!(define_split_future);
}
