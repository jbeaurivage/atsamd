//! Use the SERCOM peripheral for I2C communications.

#[cfg(any(feature = "samd11", feature = "samd21"))]
#[path = "i2c/pads_thumbv6m.rs"]
mod pads;

#[cfg(feature = "min-samd51g")]
#[path = "i2c/pads_thumbv7em.rs"]
mod pads;

pub use pads::*;

mod reg;
use reg::Registers;

mod flags;
pub use flags::*;

mod config;
pub use config::*;

pub mod impl_ehal;

use crate::{sercom::v2::*, typelevel::Sealed};
use core::{convert::TryInto, marker::PhantomData};
use num_traits::AsPrimitive;

/// Size of the SERCOM's `DATA` register
#[cfg(any(feature = "samd11", feature = "samd21"))]
pub type DataReg = u16;

/// Size of the SERCOM's `DATA` register
#[cfg(any(feature = "min-samd51g"))]
pub type DataReg = u32;

const BUS_STATE_UNKNOWN: u8 = 0;
const BUS_STATE_IDLE: u8 = 1;
const BUS_STATE_BUSY: u8 = 3;

const MASTER_ACT_READ: u8 = 2;
const MASTER_ACT_STOP: u8 = 3;

pub struct I2c<C: ValidConfig> {
    config: C,
}

impl<C: ValidConfig> I2c<C> {
    /// Obtain a pointer to the `DATA` register. Necessary for DMA transfers.
    #[cfg(feature = "dma")]
    #[inline]
    pub(crate) fn data_ptr(&self) -> *mut DataReg {
        self.config.as_ref().registers.data_ptr()
    }

    // Read the interrupt flags
    #[inline]
    pub fn read_flags(&self) -> Flags {
        self.config.as_ref().registers.read_flags()
    }

    // TODO adapt docs to i2c
    /// Clear interrupt status flags
    ///
    /// Setting the `ERROR`, `RXBRK`, `CTSIC`, `RXS`, or `TXC` flag will clear
    /// the interrupts. This function has no effect on the `DRE` or
    /// `RXC` flags.
    ///
    /// Note that only the flags pertinent to `Self`'s [`Capability`]
    /// will be cleared. The other flags will be **SILENTLY IGNORED**.
    ///
    /// * Available flags for [`Receive`] capability: `RXC`, `RXS`, `RXBRK` and
    ///   `ERROR`
    /// * Available flags for [`Transmit`] capability: `DRE` and `TXC`.
    ///   **Note**: The `CTSIC` flag can only be cleared if a `CTS` Pad was
    ///   specified in the [`Config`] via the [`clear_ctsic`](Uart::clear_ctsic)
    ///   method.
    /// * Since [`Duplex`] [`Uart`]s are [`Receive`] + [`Transmit`] they have
    ///   all flags available.
    ///
    /// **Warning:** The implementation of of
    /// [`Write::flush`](embedded_hal::serial::Write::flush) waits on and
    /// clears the `TXC` flag. Manually clearing this flag could cause it to
    /// hang indefinitely.
    #[inline]
    pub fn clear_flags(&mut self, flags: Flags) {
        // Remove flags not pertinent to Self's Capability
        let flags = Self::capability_flags(flags);
        self.config.as_mut().registers.clear_flags(flags);
    }

    /// Enable interrupts for the specified flags.
    ///
    /// Note that only the flags pertinent to `Self`'s [`Capability`]
    /// will be cleared. The other flags will be **SILENTLY IGNORED**.
    ///
    /// * Available flags for [`Receive`] capability: `RXC`, `RXS`, `RXBRK` and
    ///   `ERROR`
    /// * Available flags for [`Transmit`] capability: `DRE` and `TXC`.
    ///   **Note**: The `CTSIC` interrupt can only be enabled if a `CTS` Pad was
    ///   specified in the [`Config`] via the
    ///   [`enable_ctsic`](Uart::enable_ctsic) method.
    /// * Since [`Duplex`] [`Uart`]s are [`Receive`] + [`Transmit`] they have
    ///   all flags available.
    #[inline]
    pub fn enable_interrupts(&mut self, flags: Flags) {
        // Remove flags not pertinent to Self's Capability
        let flags = Self::capability_flags(flags);
        self.config.as_mut().registers.enable_interrupts(flags);
    }

    /// Disable interrupts for the specified flags.
    ///
    /// Note that only the flags pertinent to `Self`'s [`Capability`]
    /// will be cleared. The other flags will be **SILENTLY IGNORED**
    ///
    /// * Available flags for [`Receive`] capability: `RXC`, `RXS`, `RXBRK` and
    ///   `ERROR`
    /// * Available flags for [`Transmit`] capability: `DRE` and `TXC`.
    ///   **Note**: The `CTSIC` interrupt can only be disabled if a `CTS` Pad
    ///   was specified in the [`Config`] via the
    ///   [`disable_ctsic`](Uart::disable_ctsic) method.
    /// * Since [`Duplex`] [`Uart`]s are [`Receive`] + [`Transmit`] they have
    ///   all flags available.
    #[inline]
    pub fn disable_interrupts(&mut self, flags: Flags) {
        // Remove flags not pertinent to Self's Capability
        let flags = Self::capability_flags(flags);
        self.config.as_mut().registers.disable_interrupts(flags);
    }

    /// Read the status flags
    #[inline]
    pub fn read_status(&self) -> Status {
        self.config.as_ref().registers.read_status()
    }

    /// Clear the status flags
    ///
    /// Note that only the status flags pertinent to `Self`'s [`Capability`]
    /// will be cleared. The other stattus flags will be **SILENTLY IGNORED**.
    ///
    /// * Available status flags for [`Receive`] capability: `PERR`, `FERR`,
    ///   `BUFOVF`, `ISF` and `COLL`
    /// * [`Transmit`]-only [`Uart`]s have no clearable status flags.
    /// * Since [`Duplex`] [`Uart`]s are [`Receive`] + [`Transmit`] they have
    ///   all status flags available.
    #[inline]
    pub fn clear_status(&mut self, status: Status) {
        // Remove status flags not pertinent to Self's Capability
        let flags = Self::capability_status(status);
        self.config.as_mut().registers.clear_status(flags);
    }

    /// Reconfigure the UART.
    ///
    /// Calling this method will temporarily disable the SERCOM peripheral, as
    /// some registers are enable-protected. This may interrupt any ongoing
    /// transactions.
    ///
    /// ```
    /// use atsamd_hal::sercom::v2::uart::{BaudMode, Oversampling, Uart};
    /// uart.reconfigure(|c| c.set_run_in_standby(false));
    /// ```
    #[inline]
    pub(super) fn reconfigure<F>(&mut self, update: F)
    where
        F: FnOnce(&mut SpecificConfig<C>),
    {
        self.config.as_mut().registers.enable_peripheral(false);
        update(&mut self.config.as_mut());
        self.config.as_mut().registers.enable_peripheral(true);
    }

    /// Disable the UART peripheral and return the underlying [`Config`]
    #[inline]
    pub fn disable(self) -> C {
        let mut config = self.config;
        config.as_mut().registers.disable();
        config
    }
}
