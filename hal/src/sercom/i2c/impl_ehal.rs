//! [`embedded-hal`] trait implementations for [`I2c`]s

use super::{config::AnyConfig, flags::Error, I2c};
use crate::ehal::i2c::{self, ErrorKind, ErrorType, NoAcknowledgeSource};

impl i2c::Error for Error {
    #[allow(unreachable_patterns)]
    fn kind(&self) -> ErrorKind {
        match self {
            Error::BusError => ErrorKind::Bus,
            Error::ArbitrationLost => ErrorKind::ArbitrationLoss,
            Error::LengthError => ErrorKind::Other,
            Error::Nack => ErrorKind::NoAcknowledge(NoAcknowledgeSource::Unknown),
            Error::Timeout => ErrorKind::Other,
            // Pattern reachable when "dma" feature is enabled
            _ => ErrorKind::Other,
        }
    }
}

impl<C: AnyConfig, D> ErrorType for I2c<C, D> {
    type Error = Error;
}

impl<C: AnyConfig> i2c::I2c for I2c<C> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        /// Helper type for keeping track of the type of operation that was
        /// executed last
        #[derive(Clone, Copy)]
        enum Operation {
            Read,
            Write,
        }

        // Keep track of the last executed operation type. The method
        // specification demands, that no repeated start condition is sent
        // between adjacent operations of the same type.
        let mut last_op = None;
        for op in operations {
            match op {
                i2c::Operation::Read(buf) => {
                    if let Some(Operation::Read) = last_op {
                        self.continue_read(buf)?;
                    } else {
                        self.do_read(address, buf)?;
                        last_op = Some(Operation::Read);
                    }
                }
                i2c::Operation::Write(bytes) => {
                    if let Some(Operation::Write) = last_op {
                        self.continue_write(bytes)?;
                    } else {
                        self.do_write(address, bytes)?;
                        last_op = Some(Operation::Write);
                    }
                }
            }
        }

        self.cmd_stop();
        Ok(())
    }

    fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.do_write(address, bytes)?;
        self.cmd_stop();
        Ok(())
    }

    fn read(&mut self, address: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.do_read(address, buffer)?;
        self.cmd_stop();
        Ok(())
    }

    fn write_read(
        &mut self,
        address: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.do_write_read(address, bytes, buffer)?;
        self.cmd_stop();
        Ok(())
    }
}

#[cfg(feature = "dma")]
mod dma {
    use super::*;
    use crate::dmac::{AnyChannel, Ready};
    use crate::sercom::dma::{read_dma, write_dma, SercomPtr, SharedSliceBuffer};
    use crate::sercom::{self, Sercom};

    impl<C, S, D> I2c<C, D>
    where
        C: AnyConfig<Sercom = S>,
        S: Sercom,
    {
        fn sercom_ptr(&self) -> SercomPtr<sercom::i2c::Word> {
            SercomPtr(self.data_ptr())
        }
    }

    impl<C, D, S> i2c::I2c for I2c<C, D>
    where
        C: AnyConfig<Sercom = S>,
        S: Sercom,
        D: AnyChannel<Status = Ready>,
    {
        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            /// Helper type for keeping track of the type of operation that was
            /// executed last
            #[derive(Clone, Copy)]
            enum Operation {
                Read,
                Write,
            }

            // Keep track of the last executed operation type. The method
            // specification demands, that no repeated start condition is sent
            // between adjacent operations of the same type.
            let mut last_op = None;
            for op in operations {
                match op {
                    i2c::Operation::Read(buf) => {
                        if let Some(Operation::Read) = last_op {
                            self.continue_read(buf)?;
                        } else {
                            self.do_read(address, buf)?;
                            last_op = Some(Operation::Read);
                        }
                    }
                    i2c::Operation::Write(bytes) => {
                        if let Some(Operation::Write) = last_op {
                            self.continue_write(bytes)?;
                        } else {
                            self.do_write(address, bytes)?;
                            last_op = Some(Operation::Write);
                        }
                    }
                }
            }

            self.cmd_stop();
            Ok(())
        }

        fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            self.check_bus_status()?;
            let sercom_ptr = self.sercom_ptr();

            if bytes.is_empty() {
                return Ok(());
            }
            assert!(bytes.len() <= 255);

            self.start_dma_write(address, bytes.len() as u8);
            let mut bytes = SharedSliceBuffer::from_slice(bytes);
            let channel = self.dma_channel.as_mut();

            // SAFETY: We must make sure that any DMA transfer is complete or stopped before
            // returning.
            unsafe {
                write_dma::<_, _, S>(channel, sercom_ptr, &mut bytes)?;
            }

            while !channel.xfer_complete() {
                core::hint::spin_loop();
            }

            // Defensively disable channel
            channel.stop();

            while !self.read_status().is_idle() {
                core::hint::spin_loop();
            }

            self.read_status().check_bus_error()?;
            self.dma_channel.as_mut().xfer_success()?;
            Ok(())
        }

        fn read(&mut self, address: u8, mut buffer: &mut [u8]) -> Result<(), Self::Error> {
            self.check_bus_status()?;
            let sercom_ptr = self.sercom_ptr();

            if buffer.is_empty() {
                return Ok(());
            }
            assert!(buffer.len() <= 255);

            self.start_dma_read(address, buffer.len() as u8);
            let channel = self.dma_channel.as_mut();

            // SAFETY: We must make sure that any DMA transfer is complete or stopped before
            // returning.
            unsafe {
                read_dma::<_, _, S>(channel, sercom_ptr, &mut buffer)?;
            }

            while !channel.xfer_complete() {
                core::hint::spin_loop();
            }

            // Defensively disable channel
            channel.stop();

            self.read_status().check_bus_error()?;
            self.dma_channel.as_mut().xfer_success()?;
            Ok(())
        }

        fn write_read(
            &mut self,
            address: u8,
            bytes: &[u8],
            buffer: &mut [u8],
        ) -> Result<(), Self::Error> {
            self.write(address, bytes)?;
            self.read(address, buffer)?;
            Ok(())
        }
    }
}

impl<C: AnyConfig> crate::ehal_02::blocking::i2c::Write for I2c<C> {
    type Error = Error;

    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<(), Self::Error> {
        self.do_write(addr, bytes)?;
        self.cmd_stop();
        Ok(())
    }
}

impl<C: AnyConfig> crate::ehal_02::blocking::i2c::Read for I2c<C> {
    type Error = Error;

    fn read(&mut self, addr: u8, buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.do_read(addr, buffer)?;
        self.cmd_stop();
        Ok(())
    }
}

impl<C: AnyConfig> crate::ehal_02::blocking::i2c::WriteRead for I2c<C> {
    type Error = Error;

    fn write_read(&mut self, addr: u8, bytes: &[u8], buffer: &mut [u8]) -> Result<(), Self::Error> {
        self.do_write_read(addr, bytes, buffer)?;
        self.cmd_stop();
        Ok(())
    }
}
