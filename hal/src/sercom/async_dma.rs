//! Use the DMA Controller to perform async transfers using the SERCOM
//! peripheral
//!
//! See the [`mod@uart`], [`mod@i2c`] and [`mod@spi`] modules for the
//! corresponding DMA transfer implementations.

use cortex_m::interrupt::InterruptNumber;

use crate::{
    dmac::{self, channel::AnyChannel, Buffer, ReadyFuture, TriggerAction},
    sercom::{
        i2c::{self, I2cFuture},
        Sercom,
    },
};

unsafe impl<C, N> Buffer for I2cFuture<C, N>
where
    C: i2c::AnyConfig,
    N: InterruptNumber,
{
    type Beat = i2c::Word;

    #[inline]
    fn dma_ptr(&mut self) -> *mut Self::Beat {
        self.i2c.data_ptr()
    }

    #[inline]
    fn incrementing(&self) -> bool {
        false
    }

    #[inline]
    fn buffer_len(&self) -> usize {
        1
    }
}

impl<C, N> I2cFuture<C, N>
where
    C: i2c::AnyConfig + 'static,
    N: InterruptNumber + 'static,
{
    pub async fn write_dma<Ch, B>(
        &mut self,
        address: u8,
        buf: &mut B,
        channel: &mut Ch,
    ) -> Result<(), i2c::Error>
    where
        Ch: AnyChannel<Status = ReadyFuture>,
        B: Buffer<Beat = i2c::Word> + 'static,
    {
        self.i2c.init_dma_transfer()?;
        let len = buf.buffer_len();
        assert!(len > 0 && len <= 255);

        #[cfg(any(feature = "samd11", feature = "samd21"))]
        let trigger_action = TriggerAction::BEAT;

        #[cfg(feature = "min-samd51g")]
        let trigger_action = TriggerAction::BURST;

        self.i2c
            .config
            .as_mut()
            .registers
            .start_dma_read(address, len as u8);

        dmac::Transfer::transfer_future(
            channel,
            self,
            buf,
            C::Sercom::DMA_TX_TRIGGER,
            trigger_action,
        )
        .await
        .map_err(|_| i2c::Error::BusError)?;

        self.i2c.check_bus_status()?;
        Ok(())
    }

    pub async fn read_dma<Ch, B>(
        &mut self,
        address: u8,
        buf: &mut B,
        channel: &mut Ch,
    ) -> Result<(), i2c::Error>
    where
        Ch: AnyChannel<Status = ReadyFuture>,
        B: Buffer<Beat = i2c::Word> + 'static,
    {
        self.i2c.init_dma_transfer()?;
        let len = buf.buffer_len();
        assert!(len > 0 && len <= 255);

        #[cfg(any(feature = "samd11", feature = "samd21"))]
        let trigger_action = TriggerAction::BEAT;

        #[cfg(feature = "min-samd51g")]
        let trigger_action = TriggerAction::BURST;

        self.i2c
            .config
            .as_mut()
            .registers
            .start_dma_write(address, len as u8);

        dmac::Transfer::transfer_future(
            channel,
            buf,
            self,
            C::Sercom::DMA_RX_TRIGGER,
            trigger_action,
        )
        .await
        .map_err(|_| i2c::Error::BusError)?;

        self.i2c.check_bus_status()?;
        Ok(())
    }
}
