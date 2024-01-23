//! # Configure the SERCOM peripherals
//!
//! The SERCOM module is used to configure the SERCOM peripherals as USART, SPI
//! or I2C interfaces.
#![cfg_attr(
    feature = "thumbv7",
    doc = "
# Undocumented features
 
The ATSAMx5x chips contain certain features that aren't documented in the datasheet. 
These features are implemented in the HAL based on experimentation with certain boards
which have verifiably demonstrated that those features work as intended.

* [`UndocIoSet1`](pad::UndocIoSet1): Implement an undocumented `IoSet` for PA16, PA17,
PB22 & PB23 configured for [`Sercom1`]. The pygamer & feather_m4 use this combination,
but it is not listed as valid in the datasheet.

* [`UndocIoSet2`](pad::UndocIoSet2): Implement an undocumented `IoSet` for PA00, PA01,
PB22 & PB23 configured for [`Sercom1`]. The itsybitsy_m4 uses this combination, but it is
not listed as valid in the datasheet.

* [`PB02`] is I2C-capable according to metro_m4. As such, [`PB02`]
implements [`IsI2cPad`].

* [`PB03`] is I2C-capable according to metro_m4. As such, [`PB03`]
implements [`IsI2cPad`](pad::IsI2cPad).

[`PB02`]: crate::gpio::pin::PB02
[`PB03`]: crate::gpio::pin::PB03
[`IsI2cPad`]: pad::IsI2cPad
"
)]

use core::ops::Deref;

use paste::paste;
use seq_macro::seq;

use crate::pac::{self, sercom0, Peripherals};

#[cfg(feature = "thumbv7")]
use pac::MCLK as APB_CLK_CTRL;
#[cfg(feature = "thumbv6")]
use pac::PM as APB_CLK_CTRL;

#[cfg(feature = "dma")]
use crate::dmac::TriggerSource;

use crate::typelevel::Sealed;

pub mod pad;
pub use pad::*;

pub mod i2c;
pub mod spi;

#[deprecated(
    since = "0.17.0",
    note = "spi_future is deprecated and will be removed in a later version of atsamd_hal. Consider using the `async` APIs available in the `spi` module as a replacement."
)]
pub mod spi_future;
pub mod uart;

#[cfg(feature = "dma")]
pub mod dma;

#[cfg(all(feature = "dma", feature = "async"))]
mod async_dma;

//==============================================================================
//  Sercom
//==============================================================================

/// Type-level `enum` representing a Serial Communication Interface (SERCOM)
pub trait Sercom: Sealed + Deref<Target = sercom0::RegisterBlock> {
    /// SERCOM number
    const NUM: usize;

    /// RX Trigger source for DMA transactions
    #[cfg(feature = "dma")]
    const DMA_RX_TRIGGER: TriggerSource;

    /// TX trigger source for DMA transactions
    #[cfg(feature = "dma")]
    const DMA_TX_TRIGGER: TriggerSource;

    #[cfg(feature = "async")]
    type Interrupt: crate::async_hal::interrupts::InterruptSource;

    /// Enable the corresponding APB clock
    fn enable_apb_clock(&mut self, ctrl: &APB_CLK_CTRL);

    /// Get a reference to the sercom from a
    /// [`Peripherals`] block
    fn reg_block(peripherals: &mut Peripherals) -> &crate::pac::sercom0::RegisterBlock;

    /// Get a reference to this [`Sercom`]'s associated RX Waker
    #[cfg(feature = "async")]
    #[inline]
    fn rx_waker() -> &'static embassy_sync::waitqueue::AtomicWaker {
        &async_api::RX_WAKERS[Self::NUM]
    }

    /// Get a reference to this [`Sercom`]'s associated TX Waker
    #[cfg(feature = "async")]
    #[inline]
    fn tx_waker() -> &'static embassy_sync::waitqueue::AtomicWaker {
        &async_api::TX_WAKERS[Self::NUM]
    }
}

macro_rules! sercom {
    ( $apbmask:ident: ($start:literal, $end:literal) ) => {
        seq!(N in $start..=$end {
            paste! {
                #[cfg(feature = "has-" sercom~N)]
                use pac::SERCOM~N;
                /// Type alias for the corresponding SERCOM instance
                #[cfg(feature = "has-" sercom~N)]
                pub type Sercom~N = SERCOM~N;
                #[cfg(feature = "has-" sercom~N)]
                impl Sealed for Sercom~N {}
                #[cfg(feature = "has-" sercom~N)]
                impl Sercom for Sercom~N {
                    const NUM: usize = N;

                    #[cfg(feature = "dma")]
                    const DMA_RX_TRIGGER: TriggerSource = TriggerSource::[<SERCOM~N _RX>];

                    #[cfg(feature = "dma")]
                    const DMA_TX_TRIGGER: TriggerSource = TriggerSource::[<SERCOM~N _TX>];

                    #[cfg(all(feature = "async", feature = "thumbv6"))]
                    type Interrupt = crate::async_hal::interrupts::SERCOM~N;

                    #[cfg(all(feature = "async", feature = "thumbv7"))]
                    type Interrupt = crate::async_hal::interrupts::[<SERCOM ~N>];

                    #[inline]
                    fn enable_apb_clock(&mut self, ctrl: &APB_CLK_CTRL) {
                        ctrl.$apbmask.modify(|_, w| w.[<sercom~N _>]().set_bit());
                    }

                    #[inline]
                    fn reg_block(peripherals: &mut Peripherals) -> &crate::pac::sercom0::RegisterBlock {
                        &*peripherals.SERCOM~N
                    }

                }
            }
        });
    };
}

#[cfg(feature = "thumbv6")]
sercom!(apbcmask: (0, 5));

#[cfg(feature = "thumbv7")]
sercom!(apbamask: (0, 1));
#[cfg(feature = "thumbv7")]
sercom!(apbbmask: (2, 3));
#[cfg(feature = "thumbv7")]
sercom!(apbdmask: (4, 7));

#[allow(dead_code)]
#[cfg(all(
    feature = "has-sercom1",
    not(feature = "has-sercom3"),
    not(feature = "has-sercom5"),
    not(feature = "has-sercom7")
))]
const NUM_SERCOM: usize = 2;

#[allow(dead_code)]
#[cfg(all(
    feature = "has-sercom3",
    not(feature = "has-sercom5"),
    not(feature = "has-sercom7")
))]
const NUM_SERCOM: usize = 4;

#[allow(dead_code)]
#[cfg(all(feature = "has-sercom5", not(feature = "has-sercom7")))]
const NUM_SERCOM: usize = 6;

#[allow(dead_code)]
#[cfg(feature = "has-sercom7")]
const NUM_SERCOM: usize = 8;

#[cfg(feature = "async")]
pub(super) mod async_api {
    use embassy_sync::waitqueue::AtomicWaker;

    #[allow(clippy::declare_interior_mutable_const)]
    const NEW_WAKER: AtomicWaker = AtomicWaker::new();
    /// Waker for a RX event. By convention, if a SERCOM has only one type of
    /// event (ie, I2C), this the waker to be used.
    pub(super) static RX_WAKERS: [AtomicWaker; super::NUM_SERCOM] = [NEW_WAKER; super::NUM_SERCOM];
    /// Waker for a TX event.
    pub(super) static TX_WAKERS: [AtomicWaker; super::NUM_SERCOM] = [NEW_WAKER; super::NUM_SERCOM];
}
