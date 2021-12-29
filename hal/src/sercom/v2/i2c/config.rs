//! I2C [`Config`] definition and implementation

use super::{I2c, Registers, ValidPads};
use crate::{
    pac::{self, sercom0::i2cm::ctrla::MODE_A},
    sercom::v2::*,
    time::Hertz,
    typelevel::{Is, Sealed},
};
use core::marker::PhantomData;
use num_traits::{AsPrimitive, PrimInt};

//=============================================================================
// Operating mode
//=============================================================================

/// Type-level enum representing the I2C operating mode
///
/// See the documentation on [type-level enums] for a discussion of the pattern.
///
/// The available operating modes are [`Master`] and [`Slave`].
/// [type-level enums]: crate::typelevel#type-level-enums
pub trait OpMode: Sealed {
    /// Corresponding variant from the PAC enum
    const MODE: MODE_A;
}

/// [`OpMode`] variant for Master mode
pub enum Master {}
/// [`OpMode`] variant for Slave mode
pub enum Slave {}

impl Sealed for Master {}
impl OpMode for Master {
    const MODE: MODE_A = MODE_A::I2C_MASTER;
}

impl Sealed for Slave {}
impl OpMode for Slave {
    const MODE: MODE_A = MODE_A::I2C_SLAVE;
}

//=============================================================================
// Config
//=============================================================================

/// A configurable, disabled UART peripheral
///
/// This `struct` represents a configurable UART peripheral in its disabled
/// state. It is generic over the set of [`Pads`] and [`CharSize`].
/// Upon creation, the [`Config`] takes ownership of the
/// [`Sercom`] and resets it, returning it configured as an UART peripheral
/// with a default configuration:
///
/// * [`EightBit`]
/// * No parity
/// * One stop bit
/// * LSB-first
///
/// [`Config`] uses a builder-pattern API to configure the peripheral,
/// culminating in a call to [`enable`], which consumes the [`Config`] and
/// returns enabled [`Uart`]. The [`enable`] method is
/// restricted to [`ValidConfig`]s.
///
/// [`enable`]: Config::enable
/// [`Pads`]: super::Pads
pub struct Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
    pub(super) registers: Registers<P::Sercom>,
    pads: P,
    mode: PhantomData<M>,
    freq: Hertz,
}

impl<P: ValidPads, M: OpMode> Config<P, M> {
    /// Create a new [`Config`] in the default configuration.
    #[inline]
    fn default(sercom: P::Sercom, pads: P, freq: impl Into<Hertz>) -> Self {
        let mut registers = Registers { sercom };
        registers.swrst();
        registers.set_op_mode(Master::MODE);
        Self {
            registers,
            pads,
            mode: PhantomData,
            freq: freq.into(),
        }
    }

    /// Create a new [`Config`] in the default configuration
    ///
    /// This function will enable the corresponding APB clock, reset the
    /// [`Sercom`] peripheral, and return a [`Config`] in the default
    /// configuration. The default [`OpMode`] is [`Master`], while the default
    /// [`Size`] is an
    #[cfg_attr(
        any(feature = "samd11", feature = "samd21"),
        doc = "[`EightBit`] [`CharSize`]"
    )]
    #[cfg_attr(feature = "min-samd51g", doc = "`EightBit` `CharSize`")]
    /// for SAMD11 and SAMD21 chips or a
    #[cfg_attr(any(feature = "samd11", feature = "samd21"), doc = "`Length` of `U1`")]
    #[cfg_attr(feature = "min-samd51g", doc = "[`Length`] of `U1`")]
    /// for SAMx5x chips. Note that [`Config`] takes ownership of both the
    /// PAC [`Sercom`] struct as well as the [`Pads`].
    ///
    /// Users must configure GCLK manually. The `freq` parameter represents the
    /// GCLK frequency for this [`Sercom`] instance.
    #[inline]
    pub fn new(
        apb_clk_ctrl: &APB_CLK_CTRL,
        mut sercom: P::Sercom,
        pads: P,
        freq: impl Into<Hertz>,
    ) -> Self {
        sercom.enable_apb_clock(apb_clk_ctrl);
        Self::default(sercom, pads, freq)
    }
}

impl<P, M> Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
    /// Change the [`OpMode`]
    #[inline]
    fn op_mode<M2>(mut self) -> Config<P, M2>
    where
        M2: OpMode,
    {
        self.registers.set_op_mode(M2::MODE);

        Config {
            registers: self.registers,
            pads: self.pads,
            mode: PhantomData,
            freq: self.freq,
        }
    }

    /// Obtain a reference to the PAC `SERCOM` struct
    ///
    /// Directly accessing the `SERCOM` could break the invariants of the
    /// type-level tracking in this module, so it is unsafe.
    #[inline]
    pub unsafe fn sercom(&self) -> &P::Sercom {
        &self.registers.sercom
    }

    /// Trigger the [`Sercom`]'s SWRST and return a [`Config`] in the
    /// default configuration.
    #[inline]
    pub fn reset(self) -> Config<P, Master> {
        Config::default(self.registers.sercom, self.pads, self.freq)
    }

    /// Consume the [`Config`], reset the peripheral, and return the [`Sercom`]
    /// and [`Pads`]
    #[inline]
    pub fn free(mut self) -> (P::Sercom, P) {
        self.registers.reset();
        (self.registers.free(), self.pads)
    }

    /// Enable the I2C peripheral
    ///
    /// I2C transactions are not possible until the peripheral is enabled.
    /// This function is limited to [`ValidConfig`]s.
    #[inline]
    pub fn enable(mut self) -> I2c<Self>
    where
        Self: ValidConfig,
    {
        self.registers.enable();

        I2c { config: self }
    }
}

//=============================================================================
// AnyConfig
//=============================================================================

/// Type class for all possible [`Config`] types
///
/// This trait uses the [`AnyKind`] trait pattern to create a [type class] for
/// [`Config`] types. See the `AnyKind` documentation for more details on the
/// pattern.
///
/// In addition to the normal, `AnyKind` associated types. This trait also
/// copies the [`Sercom`], [`Capability`] and [`Word`] types, to make it easier
/// to apply bounds to these types at the next level of abstraction.
///
/// [`AnyKind`]: crate::typelevel#anykind-trait-pattern
/// [type class]: crate::typelevel#type-classes
pub trait AnyConfig: Is<Type = SpecificConfig<Self>> {
    type Sercom: Sercom;
    type Pads: ValidPads<Sercom = Self::Sercom>;
    type OpMode: OpMode;
}

/// Type alias to recover the specific [`Config`] type from an implementation of
/// [`AnyConfig`]
pub type SpecificConfig<C> = Config<<C as AnyConfig>::Pads, <C as AnyConfig>::OpMode>;

impl<P, M> Sealed for Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
}

impl<P, M> AnyConfig for Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
    type Sercom = P::Sercom;
    type Pads = P;
    type OpMode = M;
}

impl<P, M> AsRef<Self> for Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<P, M> AsMut<Self> for Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

//=============================================================================
// ValidConfig
//=============================================================================

/// Marker trait for valid SPI [`Config`]urations
///
/// A functional SPI peripheral must have, at a minimum, an SCLK pad and
/// either a Data In or a Data Out pad. Dependeing on the [`OpMode`], an SS
/// pad may also be required.
///
/// The `ValidConfig` trait is implemented only for valid combinations of
/// [`Pads`] and [`OpMode`]. No [`Config`] is valid if the SCK pad is [`NoneT`]
/// or if both the Data In and Data Out pads are `NoneT`. When in [`Master`]
/// `OpMode`, the `SS` pad must be `NoneT`, while in [`MasterHWSS`] or
/// [`Slave`] [`OpMode`], the SS pad must be [`SomePad`].
pub trait ValidConfig: AnyConfig {}

impl<P, M> ValidConfig for Config<P, M>
where
    P: ValidPads,
    M: OpMode,
{
}
