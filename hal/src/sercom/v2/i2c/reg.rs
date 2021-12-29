//! Register-level access to I2C configuration

use super::{Flags, Status};
use crate::pac;
use crate::sercom::v2::*;
use crate::time::Hertz;

const MASTER_MODE_BITS: u8 = 0x5;

pub(super) struct Registers<S: Sercom> {
    sercom: S,
}

// SAFETY: It is safe to implement Sync for Registers, because it erases the
// interior mutability of the PAC SERCOM struct.
unsafe impl<S: Sercom> Sync for Registers<S> {}

impl<S: Sercom> Registers<S> {
    /// Create a new `Registers` instance
    #[inline]
    pub(super) fn new(sercom: S) -> Self {
        Self { sercom }
    }

    /// Helper function to access the underlying `I2CM` from the given `SERCOM`
    #[inline]
    fn i2c_master(&self) -> &pac::sercom0::I2CM {
        self.sercom.i2cm()
    }

    #[cfg(feature = "dma")]
    /// Get a pointer to the `DATA` register
    pub(super) fn data_ptr<T>(&self) -> *mut T {
        self.i2c_master().data.as_ptr() as *mut _
    }

    /// Free the `Registers` struct and return the underlying `Sercom` instance
    #[inline]
    pub(super) fn free(self) -> S {
        self.sercom
    }

    /// Reset the SERCOM peripheral
    #[inline]
    pub(super) fn swrst(&mut self) {
        self.i2c_master().ctrla.write(|w| w.swrst().set_bit());
        while self.i2c_master().syncbusy.read().swrst().bit_is_set() {}
    }

    /// Configure the SERCOM to use I2C master mode
    #[inline]
    pub(super) fn set_op_mode(&mut self, mode: pac::sercom0::i2cm::ctrla::MODE_A) {
        self.i2c_master()
            .ctrla
            .modify(|_, w| w.mode().variant(mode));
    }

    // Run in standby mode
    ///
    /// When set, the I2C peripheral will run in standby mode. See the
    /// datasheet for more details.
    #[inline]
    pub(super) fn set_run_in_standby(&mut self, set: bool) {
        self.i2c_master().ctrla.modify(|_, w| w.runstdby().bit(set));
    }

    /// Get the current run in standby mode
    #[inline]
    pub(super) fn get_run_in_standby(&self) -> bool {
        self.i2c_master().ctrla.read().runstdby().bit()
    }

    /// Clear specified interrupt flags
    #[inline]
    pub(super) fn clear_flags(&mut self, flags: Flags) {
        self.i2c_master()
            .intflag
            .modify(|_, w| unsafe { w.bits(flags.bits()) });
    }

    /// Read interrupt flags
    #[inline]
    pub(super) fn read_flags(&self) -> Flags {
        Flags::from_bits_truncate(self.i2c_master().intflag.read().bits())
    }

    /// Enable specified interrupts
    #[inline]
    pub(super) fn enable_interrupts(&mut self, flags: Flags) {
        self.i2c_master()
            .intenset
            .write(|w| unsafe { w.bits(flags.bits()) });
    }

    /// Disable specified interrupts
    #[inline]
    pub(super) fn disable_interrupts(&mut self, flags: Flags) {
        self.i2c_master()
            .intenclr
            .write(|w| unsafe { w.bits(flags.bits()) });
    }

    /// Clear specified status flags
    #[inline]
    pub(super) fn clear_status(&mut self, status: Status) {
        self.i2c_master()
            .status
            .modify(|_, w| unsafe { w.bits(status.bits()) });
    }

    /// Read status flags
    #[inline]
    pub(super) fn read_status(&self) -> Status {
        Status::from_bits_truncate(self.i2c_master().status.read().bits())
    }

    /// Read from the `DATA` register
    #[inline]
    pub(super) unsafe fn read_data(&mut self) -> super::DataReg {
        self.i2c_master().data.read().data().bits()
    }

    /// Write to the `DATA` register
    #[inline]
    pub(super) unsafe fn write_data(&mut self, data: super::DataReg) {
        self.i2c_master().data.write(|w| w.data().bits(data))
    }

    /// Enable the I2C peripheral
    ///
    /// I2C transactions are not possible until the peripheral is enabled.
    #[inline]
    pub(super) fn enable(&mut self) {
        // Globally enable peripheral
        self.enable_peripheral(true);
    }

    #[inline]
    pub(super) fn disable(&mut self) {
        self.enable_peripheral(false);
    }

    /// Enable or disable the SERCOM peripheral, and wait for the ENABLE bit to
    /// synchronize.
    pub(super) fn enable_peripheral(&mut self, enable: bool) {
        self.i2c_master()
            .ctrla
            .modify(|_, w| w.enable().bit(enable));
        while self.i2c_master().syncbusy.read().enable().bit_is_set() {}
    }
}
