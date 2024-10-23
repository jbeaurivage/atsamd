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

impl<C: AnyConfig, D> I2c<C, D> {
    fn transaction_byte_by_byte(
        &mut self,
        address: u8,
        operations: &mut [i2c::Operation<'_>],
    ) -> Result<(), Error> {
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
}

impl<C: AnyConfig> i2c::I2c for I2c<C> {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.transaction_byte_by_byte(address, operations)?;
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
    use i2c::Operation;

    use super::*;
    use crate::dmac::{AnyChannel, DmacDescriptor, Ready};
    use crate::sercom::dma::{
        read_dma, read_dma_linked, write_dma, write_dma_linked, SercomPtr, SharedSliceBuffer,
    };
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
            use i2c::Operation;

            const NUM_LINKED_TRANSFERS: usize = 16;

            if operations.is_empty() {
                return Ok(());
            }

            let mut sercom_ptr = self.sercom_ptr();

            // Reserve some space for linked DMA transfer descriptors.
            // Uses 256 bytes of memory.
            //
            // In practice this means that we can only support 17 continuously
            // linked operations of the same type (R/W) before having to issue
            // an I2C STOP. DMA-enabled I2C transfers automatically issue stop
            // commands, and there is no way to turn off that behaviour.
            //
            //  In the event that we have more than 17 contiguous operations of
            //  the same type, we must revert to the byte-by-byte I2C implementations.
            let mut descriptors = heapless::Vec::<DmacDescriptor, NUM_LINKED_TRANSFERS>::new();

            let mut i = 0;

            // Look ahead and find how many consecutive operations of the same type we have
            while i < operations.len() {
                // First operation in this group
                let first_op = &operations[i];
                let mut contiguous_count = 0;

                for op in &operations[i..] {
                    if matches!(
                        (first_op, op),
                        (Operation::Write(_), Operation::Write(_))
                            | (Operation::Read(_), Operation::Read(_))
                    ) {
                        contiguous_count += 1;
                    } else {
                        break;
                    }
                }

                let ops_group = &mut operations[i..i + contiguous_count];

                // Default to byte-by-byte impl if we have more than 17 continuous operations, as we
                // would overflow our DMA linked transfers otherwise.
                if ops_group.len() > NUM_LINKED_TRANSFERS {
                    self.transaction_byte_by_byte(address, ops_group)?;
                } else {
                    // Setup all linked descriptors
                    let mut num_bytes_to_process = 0;
                    for op in ops_group.iter_mut().skip(1) {
                        match op {
                            Operation::Read(ref mut buffer) => {
                                if buffer.is_empty() {
                                    continue;
                                }
                                descriptors
                                    .push(DmacDescriptor::default())
                                    .unwrap_or_else(|_| panic!("BUG: DMAC descriptors overflow"));
                                let last_descriptor = descriptors.last_mut().unwrap();
                                let next_ptr =
                                    (last_descriptor as *mut DmacDescriptor).wrapping_add(1);

                                crate::dmac::Transfer::<D, _>::link_descriptor(
                                    last_descriptor,
                                    &mut sercom_ptr,
                                    buffer,
                                    // Always link the next descriptor. We then set the last
                                    // transfer's link pointer to null lower down in the code.
                                    next_ptr,
                                );
                                num_bytes_to_process += buffer.len();
                            }
                            Operation::Write(bytes) => {
                                if bytes.is_empty() {
                                    continue;
                                }
                                descriptors
                                    .push(DmacDescriptor::default())
                                    .unwrap_or_else(|_| panic!("BUG: DMAC descriptors overflow"));
                                let last_descriptor = descriptors.last_mut().unwrap();
                                let next_ptr =
                                    (last_descriptor as *mut DmacDescriptor).wrapping_add(1);

                                let bytes_len = bytes.len();
                                let mut bytes = SharedSliceBuffer::from_slice(bytes);
                                crate::dmac::Transfer::<D, _>::link_descriptor(
                                    last_descriptor,
                                    &mut bytes,
                                    &mut sercom_ptr,
                                    // Always link the next descriptor. We then set the last
                                    // transfer's link pointer to null lower down in the code.
                                    next_ptr,
                                );

                                num_bytes_to_process += bytes_len;
                            }
                        }
                    }

                    // Set the last descriptor to a null pointer to stop the transfer, and avoid
                    // buffer overflow UB.
                    if let Some(d) = descriptors.last_mut() {
                        d.set_next_descriptor(core::ptr::null_mut());
                    }

                    // Now setup and perform the actual transfer
                    match ops_group.first_mut().unwrap() {
                        Operation::Read(ref mut buffer) => {
                            self.check_bus_status()?;
                            let sercom_ptr = self.sercom_ptr();

                            if buffer.is_empty() {
                                return Ok(());
                            }
                            num_bytes_to_process += buffer.len();
                            assert!(
                            num_bytes_to_process <= 255,
                            "Cannot read/write more than 255 bytes in a continuous DMA operation"
                        );

                            self.start_dma_read(address, num_bytes_to_process as u8);
                            let channel = self.dma_channel.as_mut();

                            // SAFETY: We must make sure that any DMA transfer is complete or
                            // stopped before returning.
                            unsafe {
                                read_dma_linked::<_, _, S>(
                                    channel,
                                    sercom_ptr,
                                    buffer,
                                    // Only link a descriptor if it exists
                                    descriptors.first_mut(),
                                )?;
                            }

                            while !channel.xfer_complete() {
                                core::hint::spin_loop();
                            }

                            // Defensively disable channel
                            channel.stop();

                            self.read_status().check_bus_error()?;
                            self.dma_channel.as_mut().xfer_success()?;
                        }
                        Operation::Write(bytes) => {
                            self.check_bus_status()?;
                            let sercom_ptr = self.sercom_ptr();

                            if bytes.is_empty() {
                                return Ok(());
                            }
                            num_bytes_to_process += bytes.len();
                            assert!(
                            num_bytes_to_process <= 255,
                            "Cannot read/write more than 255 bytes in a continuous DMA operation"
                        );

                            self.start_dma_write(address, num_bytes_to_process as u8);
                            let mut bytes = SharedSliceBuffer::from_slice(bytes);
                            let channel = self.dma_channel.as_mut();

                            // SAFETY: We must make sure that any DMA transfer is complete or
                            // stopped before returning.
                            unsafe {
                                write_dma_linked::<_, _, S>(
                                    channel,
                                    sercom_ptr,
                                    &mut bytes,
                                    // Only link a descriptor if it exists
                                    descriptors.first_mut(),
                                )?;
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
                        }
                    }
                }

                i += contiguous_count;
                descriptors.clear();
            }

            Ok(())
        }

        fn write(&mut self, address: u8, bytes: &[u8]) -> Result<(), Self::Error> {
            self.check_bus_status()?;
            let sercom_ptr = self.sercom_ptr();

            if bytes.is_empty() {
                return Ok(());
            }

            // Default to byte-by-byte impl if we exceed the I2C-DMA transfer limitations
            if bytes.len() > 255 {
                return self.transaction_byte_by_byte(address, &mut [Operation::Write(bytes)]);
            }

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

            // Default to byte-by-byte impl if we exceed the I2C-DMA transfer limitations
            if buffer.len() > 255 {
                return self.transaction_byte_by_byte(address, &mut [Operation::Read(buffer)]);
            }

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
