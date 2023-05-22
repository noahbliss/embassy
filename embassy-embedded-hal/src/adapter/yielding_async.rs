use embassy_futures::yield_now;

/// Wrapper that yields for each operation to the wrapped instance
///
/// This can be used in combination with BlockingAsync<T> to enforce yields
/// between long running blocking operations.
pub struct YieldingAsync<T> {
    wrapped: T,
}

impl<T> YieldingAsync<T> {
    /// Create a new instance of a wrapper that yields after each operation.
    pub fn new(wrapped: T) -> Self {
        Self { wrapped }
    }
}

//
// I2C implementations
//
impl<T> embedded_hal_1::i2c::ErrorType for YieldingAsync<T>
where
    T: embedded_hal_1::i2c::ErrorType,
{
    type Error = T::Error;
}

impl<T> embedded_hal_async::i2c::I2c for YieldingAsync<T>
where
    T: embedded_hal_async::i2c::I2c,
{
    async fn read(&mut self, address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
        self.wrapped.read(address, read).await?;
        yield_now().await;
        Ok(())
    }

    async fn write(&mut self, address: u8, write: &[u8]) -> Result<(), Self::Error> {
        self.wrapped.write(address, write).await?;
        yield_now().await;
        Ok(())
    }

    async fn write_read(&mut self, address: u8, write: &[u8], read: &mut [u8]) -> Result<(), Self::Error> {
        self.wrapped.write_read(address, write, read).await?;
        yield_now().await;
        Ok(())
    }

    async fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal_1::i2c::Operation<'_>],
    ) -> Result<(), Self::Error> {
        self.wrapped.transaction(address, operations).await?;
        yield_now().await;
        Ok(())
    }
}

//
// SPI implementations
//

impl<T> embedded_hal_async::spi::ErrorType for YieldingAsync<T>
where
    T: embedded_hal_async::spi::ErrorType,
{
    type Error = T::Error;
}

impl<T> embedded_hal_async::spi::SpiBus<u8> for YieldingAsync<T>
where
    T: embedded_hal_async::spi::SpiBus,
{
    async fn transfer<'a>(&'a mut self, read: &'a mut [u8], write: &'a [u8]) -> Result<(), Self::Error> {
        self.wrapped.transfer(read, write).await?;
        yield_now().await;
        Ok(())
    }

    async fn transfer_in_place<'a>(&'a mut self, words: &'a mut [u8]) -> Result<(), Self::Error> {
        self.wrapped.transfer_in_place(words).await?;
        yield_now().await;
        Ok(())
    }
}

impl<T> embedded_hal_async::spi::SpiBusFlush for YieldingAsync<T>
where
    T: embedded_hal_async::spi::SpiBusFlush,
{
    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.wrapped.flush().await?;
        yield_now().await;
        Ok(())
    }
}

impl<T> embedded_hal_async::spi::SpiBusWrite<u8> for YieldingAsync<T>
where
    T: embedded_hal_async::spi::SpiBusWrite<u8>,
{
    async fn write(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.wrapped.write(data).await?;
        yield_now().await;
        Ok(())
    }
}

impl<T> embedded_hal_async::spi::SpiBusRead<u8> for YieldingAsync<T>
where
    T: embedded_hal_async::spi::SpiBusRead<u8>,
{
    async fn read(&mut self, data: &mut [u8]) -> Result<(), Self::Error> {
        self.wrapped.read(data).await?;
        yield_now().await;
        Ok(())
    }
}

///
/// NOR flash implementations
///
impl<T: embedded_storage::nor_flash::ErrorType> embedded_storage::nor_flash::ErrorType for YieldingAsync<T> {
    type Error = T::Error;
}

impl<T: embedded_storage_async::nor_flash::ReadNorFlash> embedded_storage_async::nor_flash::ReadNorFlash
    for YieldingAsync<T>
{
    const READ_SIZE: usize = T::READ_SIZE;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.wrapped.read(offset, bytes).await?;
        Ok(())
    }

    fn capacity(&self) -> usize {
        self.wrapped.capacity()
    }
}

impl<T: embedded_storage_async::nor_flash::NorFlash> embedded_storage_async::nor_flash::NorFlash for YieldingAsync<T> {
    const WRITE_SIZE: usize = T::WRITE_SIZE;
    const ERASE_SIZE: usize = T::ERASE_SIZE;

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.wrapped.write(offset, bytes).await?;
        yield_now().await;
        Ok(())
    }

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        // Yield between each actual erase
        for from in (from..to).step_by(T::ERASE_SIZE) {
            let to = core::cmp::min(from + T::ERASE_SIZE as u32, to);
            self.wrapped.erase(from, to).await?;
            yield_now().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use embedded_storage_async::nor_flash::NorFlash;

    use super::*;

    extern crate std;

    #[derive(Default)]
    struct FakeFlash(Vec<(u32, u32)>);

    impl embedded_storage::nor_flash::ErrorType for FakeFlash {
        type Error = std::convert::Infallible;
    }

    impl embedded_storage_async::nor_flash::ReadNorFlash for FakeFlash {
        const READ_SIZE: usize = 1;

        async fn read(&mut self, _offset: u32, _bytes: &mut [u8]) -> Result<(), Self::Error> {
            unimplemented!()
        }

        fn capacity(&self) -> usize {
            unimplemented!()
        }
    }

    impl embedded_storage_async::nor_flash::NorFlash for FakeFlash {
        const WRITE_SIZE: usize = 4;
        const ERASE_SIZE: usize = 128;

        async fn write(&mut self, _offset: u32, _bytes: &[u8]) -> Result<(), Self::Error> {
            unimplemented!()
        }

        async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            self.0.push((from, to));
            Ok(())
        }
    }

    #[futures_test::test]
    async fn can_erase() {
        let fake = FakeFlash::default();
        let mut yielding = YieldingAsync::new(fake);

        yielding.erase(0, 256).await.unwrap();

        let fake = yielding.wrapped;
        assert_eq!(2, fake.0.len());
        assert_eq!((0, 128), fake.0[0]);
        assert_eq!((128, 256), fake.0[1]);
    }

    #[futures_test::test]
    async fn can_erase_wrong_erase_size() {
        let fake = FakeFlash::default();
        let mut yielding = YieldingAsync::new(fake);

        yielding.erase(0, 257).await.unwrap();

        let fake = yielding.wrapped;
        assert_eq!(3, fake.0.len());
        assert_eq!((0, 128), fake.0[0]);
        assert_eq!((128, 256), fake.0[1]);
        assert_eq!((256, 257), fake.0[2]);
    }
}
