use super::*;
use crate::ehal::spi::{self, ErrorType, SpiBus};
#[allow(unused_imports)]
use crate::ehal_02::{blocking, serial};
use num_traits::PrimInt;

#[hal_module(
    any("sercom0-d11", "sercom0-d21") => "impl_ehal_thumbv6m.rs",
    "sercom0-d5x" => "impl_ehal_thumbv7em.rs"
)]
pub mod impl_ehal_02 {}

impl spi::Error for Error {
    #[allow(unreachable_patterns)]
    fn kind(&self) -> crate::ehal::spi::ErrorKind {
        match self {
            Error::Overflow => crate::ehal::spi::ErrorKind::Overrun,
            Error::LengthError => crate::ehal::spi::ErrorKind::Other,
            // Pattern reachable when "dma" feature is enabled
            _ => crate::ehal::spi::ErrorKind::Other,
        }
    }
}

impl<C, D, R, T> ErrorType for Spi<C, D, R, T>
where
    C: ValidConfig,
    D: Capability,
{
    type Error = Error;
}

impl<P, M, C, R, T> Spi<Config<P, M, C>, Duplex, R, T>
where
    Config<P, M, C>: ValidConfig,
    P: ValidPads,
    M: MasterMode,
    C: Size + 'static,
    C::Word: PrimInt + AsPrimitive<DataWidth>,
    DataWidth: AsPrimitive<C::Word>,
{
    /// Read and write a single word to the bus simultaneously.
    fn transfer_word_in_place(&mut self, word: C::Word) -> Result<C::Word, Error> {
        self.block_on_flags(Flags::DRE)?;

        unsafe {
            self.write_data(word.as_());
        }

        self.block_on_flags(Flags::TXC | Flags::RXC)?;
        let word = unsafe { self.read_data().as_() };
        Ok(word)
    }

    /// Perform a transfer, word by word.
    ///
    /// No-op words will be written if `read` is longer than `write`. Extra
    /// words are ignored if `write` is longer than `read`.
    fn transfer_word_by_word(
        &mut self,
        read: &mut [C::Word],
        write: &[C::Word],
    ) -> Result<(), Error> {
        let nop_word = self.config.nop_word;
        for (r, w) in read
            .iter_mut()
            .zip(write.iter().chain(core::iter::repeat(&nop_word.as_())))
        {
            *r = self.transfer_word_in_place(*w)?;
        }

        Ok(())
    }
}

impl<P, M, C> SpiBus<Word<C>> for Spi<Config<P, M, C>, Duplex>
where
    Config<P, M, C>: ValidConfig,
    P: ValidPads,
    M: MasterMode,
    C: Size + 'static,
    C::Word: PrimInt + AsPrimitive<DataWidth> + Copy,
    DataWidth: AsPrimitive<C::Word>,
{
    fn read(&mut self, words: &mut [Word<C>]) -> Result<(), Self::Error> {
        for word in words.iter_mut() {
            *word = self.transfer_word_in_place(self.config.nop_word.as_())?;
        }

        Ok(())
    }

    #[inline]
    fn write(&mut self, words: &[Word<C>]) -> Result<(), Self::Error> {
        // We are `Duplex`, so we must receive as many words as we send,
        // otherwise we could trigger an overflow
        for word in words {
            let _ = self.transfer_word_in_place(*word)?;
        }
        Ok(())
    }

    #[inline]
    fn transfer(&mut self, read: &mut [Word<C>], write: &[Word<C>]) -> Result<(), Self::Error> {
        self.transfer_word_by_word(read, write)
    }

    #[inline]
    fn transfer_in_place<'w>(&mut self, words: &mut [Word<C>]) -> Result<(), Self::Error> {
        for word in words {
            let read = self.transfer_word_in_place(*word)?;
            *word = read;
        }

        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> Result<(), Error> {
        // Ignore buffer overflow errors
        let _ = self.block_on_flags(Flags::TXC);
        Ok(())
    }
}

#[cfg(feature = "dma")]
mod dma {
    use super::*;
    use crate::dmac::{channel::Ready, AnyChannel, Beat, Buffer, DEFAULT_DESCRIPTOR};
    use crate::sercom::dma::{
        read_dma, read_dma_linked, write_dma, write_dma_linked, SercomPtr, SharedSliceBuffer,
    };

    struct SinkSourceBuffer<'a, T: Beat> {
        word: &'a mut T,
        length: usize,
    }

    /// Sink/source buffer to use for unidirectional SPI-DMA transfers.
    ///
    /// When reading/writing from a [`Duplex`] [`SpiFuture`] with DMA enabled,
    /// we must always read and write the same number of words, regardless of
    /// whether we care about the result (ie, for [`write`], we discard the read
    /// words, whereas for [`read`], we must send a no-op word).
    ///
    /// This [`Buffer`] implementation provides a source/sink for a single word,
    /// but with a variable length.
    impl<'a, T: Beat> SinkSourceBuffer<'a, T> {
        fn new(word: &'a mut T, length: usize) -> Self {
            Self { word, length }
        }
    }
    unsafe impl<'a, T: Beat> Buffer for SinkSourceBuffer<'a, T> {
        type Beat = T;
        fn incrementing(&self) -> bool {
            false
        }
        fn buffer_len(&self) -> usize {
            self.length
        }
        fn dma_ptr(&mut self) -> *mut Self::Beat {
            self.word as *mut _
        }
    }

    impl<P, M, Z, D, R, T> Spi<Config<P, M, Z>, D, R, T>
    where
        P: ValidPads,
        M: OpMode,
        Z: Size,
        Config<P, M, Z>: ValidConfig,
        D: Capability,
        Z::Word: Beat,
    {
        fn sercom_ptr(&self) -> SercomPtr<Z::Word> {
            SercomPtr(self.config.regs.spi().data().as_ptr() as *mut _)
        }
    }

    impl<P, M, S, C, R, T, W> Spi<Config<P, M, C>, Duplex, R, T>
    where
        Config<P, M, C>: ValidConfig<Sercom = S, Word = W>,
        S: Sercom,
        P: ValidPads,
        M: MasterMode,
        C: Size<Word = W> + 'static,
        C::Word: PrimInt + AsPrimitive<DataWidth> + Beat,
        W: Beat,
        DataWidth: AsPrimitive<C::Word>,
        R: AnyChannel<Status = Ready>,
        T: AnyChannel<Status = Ready>,
    {
        #[inline]
        fn transfer_blocking<Source: Buffer<Beat = W>, Dest: Buffer<Beat = W>>(
            &mut self,
            dest: &mut Dest,
            source: &mut Source,
        ) -> Result<(), Error> {
            let sercom_ptr = self.sercom_ptr();
            let rx = self.rx_channel.as_mut();
            let tx = self.tx_channel.as_mut();

            // SAFETY: We make sure that any DMA transfer is complete or stopped before
            // returning. The order of operations is important; the RX transfer
            // must be ready to receive before the TX transfer is initiated.
            unsafe {
                read_dma::<_, _, S>(rx, sercom_ptr.clone(), dest)?;

                // We can't use the ? operator here; we need to stop the read transfer before
                // returning.
                if let Err(e) = write_dma::<_, _, S>(tx, sercom_ptr, source) {
                    rx.stop();
                    return Err(e.into());
                };
            }

            while !(rx.xfer_complete() && tx.xfer_complete()) {
                core::hint::spin_loop();
            }

            // Defensively disable channels
            tx.stop();
            rx.stop();
            self.block_on_flags(Flags::TXC | Flags::RXC)?;
            self.rx_channel
                .as_mut()
                .xfer_success()
                .and(self.tx_channel.as_mut().xfer_success())?;
            Ok(())
        }
    }

    impl<P, M, S, C, R, T, W> SpiBus<Word<C>> for Spi<Config<P, M, C>, Duplex, R, T>
    where
        Config<P, M, C>: ValidConfig<Sercom = S, Word = W>,
        S: Sercom,
        P: ValidPads,
        M: MasterMode,
        C: Size<Word = W> + 'static,
        C::Word: PrimInt + AsPrimitive<DataWidth> + Beat,
        W: Beat,
        DataWidth: AsPrimitive<C::Word>,
        R: AnyChannel<Status = Ready>,
        T: AnyChannel<Status = Ready>,
    {
        #[hal_macro_helper]
        fn read(&mut self, mut words: &mut [C::Word]) -> Result<(), Self::Error> {
            // TODO: make this configurable
            let mut source_word = 0x00.as_();
            let mut source = SinkSourceBuffer::new(&mut source_word, words.len());

            self.transfer_blocking(&mut words, &mut source)
        }

        #[inline]
        fn write(&mut self, words: &[C::Word]) -> Result<(), Self::Error> {
            // Use a random value as the sink word since we're just going to discard it
            let mut sink_word = 0xFF.as_();
            let mut sink = SinkSourceBuffer::new(&mut sink_word, words.len());
            let mut words = SharedSliceBuffer::from_slice(words);

            self.transfer_blocking(&mut sink, &mut words)
        }

        #[inline]
        fn transfer(
            &mut self,
            mut read: &mut [C::Word],
            write: &[C::Word],
        ) -> Result<(), Self::Error> {
            use core::cmp::Ordering;

            // No work to do here
            if write.is_empty() && read.is_empty() {
                return Ok(());
            }

            // Handle 0-length special cases
            if write.is_empty() {
                return self.read(read);
            } else if read.is_empty() {
                return self.write(write);
            }

            // Reserve space for a DMAC SRAM descriptor if we need to make a linked
            // transfer. Must not be dropped until all transfers have completed
            // or have been stopped.
            let mut linked_descriptor = DEFAULT_DESCRIPTOR;
            // TODO: make this configurable
            // Must not be dropped until all transfers have completed or have been stopped.
            let mut source_sink_word = 0x00.as_();
            let mut sercom_ptr = self.sercom_ptr();

            let (read_link, write_link) = match read.len().cmp(&write.len()) {
                Ordering::Equal => {
                    let mut write = SharedSliceBuffer::from_slice(write);
                    return self.transfer_blocking(&mut read, &mut write);
                }

                // `read` is shorter; link transfer to sink incoming words after the buffer has been
                // filled.
                Ordering::Less => {
                    let mut sink =
                        SinkSourceBuffer::new(&mut source_sink_word, write.len() - read.len());
                    crate::dmac::Transfer::<R, _>::link_descriptor(
                        &mut linked_descriptor,
                        &mut sercom_ptr,
                        &mut sink,
                        // Add a null descriptor pointer to end the transfer.
                        core::ptr::null_mut(),
                    );

                    (Some(&mut linked_descriptor), None)
                }

                // `write` is shorter; link transfer to send NOP word after the buffer has been
                // exhausted.
                Ordering::Greater => {
                    let mut source =
                        SinkSourceBuffer::new(&mut source_sink_word, read.len() - write.len());
                    crate::dmac::Transfer::<R, _>::link_descriptor(
                        &mut linked_descriptor,
                        &mut source,
                        &mut sercom_ptr,
                        // Add a null descriptor pointer to end the transfer.
                        core::ptr::null_mut(),
                    );

                    (None, Some(&mut linked_descriptor))
                }
            };

            let rx = self.rx_channel.as_mut();
            let tx = self.tx_channel.as_mut();

            let mut write = SharedSliceBuffer::from_slice(write);

            // SAFETY: We make sure that any DMA transfer is complete or stopped before
            // returning. The order of operations is important; the RX transfer
            // must be ready to receive before the TX transfer is initiated.
            unsafe {
                read_dma_linked::<_, _, S>(rx, sercom_ptr.clone(), &mut read, read_link)?;

                // We can't use the ? operator here; we need to stop the read transfer
                // before returning.
                if let Err(e) = write_dma_linked::<_, _, S>(tx, sercom_ptr, &mut write, write_link)
                {
                    rx.stop();
                    return Err(e.into());
                };
            }

            while !(rx.xfer_complete() && tx.xfer_complete()) {
                core::hint::spin_loop();
            }

            // Defensively disable channels
            tx.stop();
            rx.stop();
            self.block_on_flags(Flags::TXC | Flags::RXC)?;
            self.rx_channel
                .as_mut()
                .xfer_success()
                .and(self.tx_channel.as_mut().xfer_success())?;
            Ok(())
        }

        #[inline]
        fn transfer_in_place<'w>(&mut self, mut words: &mut [C::Word]) -> Result<(), Self::Error> {
            let mut write_buf = SharedSliceBuffer::from_slice(words);
            self.transfer_blocking(&mut words, &mut write_buf)
        }

        #[inline]
        fn flush(&mut self) -> Result<(), Error> {
            // Ignore buffer overflow errors
            let _ = self.block_on_flags(Flags::TXC | Flags::RXC);
            Ok(())
        }
    }
}
