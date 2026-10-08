use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{
    Error as FsError, Mode as FileMode, SdCard, SdCardError, TimeSource, Timestamp, VolumeIdx,
    VolumeManager,
};
use esp_hal::{
    delay::Delay,
    gpio::{
        interconnect::PeripheralInput, interconnect::PeripheralOutput, Level, Output, OutputConfig,
        OutputPin,
    },
    spi::{
        master::{Config as SpiConfig, Instance, Spi},
        Mode,
    },
    time::Rate,
    Blocking,
};
use esp_println::println;

use sporos_app::sd::SdError;

/// SD cards must be brought up at 100-400 kHz; only after they answer may the
/// clock go up.
const INIT_FREQUENCY: Rate = Rate::from_khz(400);

/// Within what every SD card supports in SPI mode (25 MHz), with margin.
const RUN_FREQUENCY: Rate = Rate::from_mhz(20);

type Card<'d> = SdCard<ExclusiveDevice<Spi<'d, Blocking>, Output<'d>, Delay>, Delay>;
type Volumes<'d> = VolumeManager<Card<'d>, FixedTime>;

/// Every file gets the same timestamp. There is no RTC to give a real one, and
/// a real one would record when a secret was written.
struct FixedTime;

impl TimeSource for FixedTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

/// A FAT-formatted card on its own SPI bus, read and written as whole files in
/// the root directory. Names are 8.3 (`SEED.TXT`): no long filenames.
///
/// Nothing touches the card until it is used, and it is brought up again after
/// any failure, so a card inserted after boot — or pulled and put back — works
/// without a reset.
pub struct SdStorage<'d> {
    /// Taken out for the length of each operation, see [`Self::with_volumes`].
    card: Option<Card<'d>>,
    /// Whether the card answered its start-up and the bus is at full speed.
    ready: bool,
}

impl<'d> SdStorage<'d> {
    pub fn new(
        spi: impl Instance + 'd,
        sck: impl PeripheralOutput<'d>,
        mosi: impl PeripheralOutput<'d>,
        miso: impl PeripheralInput<'d>,
        cs: impl OutputPin + 'd,
    ) -> Self {
        let bus = Spi::new(spi, bus_config(INIT_FREQUENCY))
            .expect("invalid SD SPI configuration")
            .with_sck(sck)
            .with_mosi(mosi)
            .with_miso(miso);

        let cs = Output::new(cs, Level::High, OutputConfig::default());
        let device = ExclusiveDevice::new(bus, cs, Delay::new()).expect("failed to drive CS high");

        Self {
            card: Some(SdCard::new(device, Delay::new())),
            ready: false,
        }
    }

    /// Writes `data` to `name`, replacing whatever was there.
    pub fn store(&mut self, name: &str, data: &[u8]) -> Result<(), SdError> {
        self.with_volumes(|volumes| {
            let volume = volumes.open_volume(VolumeIdx(0))?;
            let root = volume.open_root_dir()?;
            let file = root.open_file_in_dir(name, FileMode::ReadWriteCreateOrTruncate)?;
            file.write(data)?;
            // Closing is what writes the directory entry; dropping would lose
            // the error.
            file.close()
        })
    }

    /// Reads `name` into `buf` and returns how many bytes it holds. Anything
    /// past `buf.len()` is left unread.
    pub fn load(&mut self, name: &str, buf: &mut [u8]) -> Result<usize, SdError> {
        self.with_volumes(|volumes| {
            let volume = volumes.open_volume(VolumeIdx(0))?;
            let root = volume.open_root_dir()?;
            let file = root.open_file_in_dir(name, FileMode::ReadOnly)?;

            let mut len = 0;
            while len < buf.len() && !file.is_eof() {
                len += file.read(&mut buf[len..])?;
            }
            Ok(len)
        })
    }

    /// Runs one operation on a filesystem mounted for it alone.
    ///
    /// The volume manager keeps the last block it read, which would be stale
    /// after a card swap and could hold part of a phrase. A fresh one per
    /// operation, freed straight after, keeps neither around.
    fn with_volumes<R>(
        &mut self,
        operation: impl FnOnce(&Volumes<'d>) -> Result<R, FsError<SdCardError>>,
    ) -> Result<R, SdError> {
        self.bring_up()?;

        let card = self.card.take().expect("the card is always put back");
        let volumes = VolumeManager::new(card, FixedTime);
        let result = operation(&volumes);
        let (card, _) = volumes.free();
        self.card = Some(card);

        result.map_err(|err| {
            // Kinds only, never contents: nothing here carries file data.
            println!("sd card error: {:?}", err);

            match err {
                FsError::NotFound => SdError::NotFound,
                // Whatever went wrong, the card is started from scratch next
                // time: it may have been pulled mid-operation.
                FsError::DeviceError(_) => {
                    self.reset();
                    SdError::Failed
                }
                _ => SdError::Failed,
            }
        })
    }

    /// Starts the card if it is not running yet: slow clock, the start-up
    /// handshake, then full speed.
    fn bring_up(&mut self) -> Result<(), SdError> {
        if self.ready {
            return Ok(());
        }

        let card = self.card.as_mut().expect("the card is always put back");
        card.spi(|device| {
            device
                .bus_mut()
                .apply_config(&bus_config(INIT_FREQUENCY))
                .expect("invalid SD SPI configuration")
        });

        match card.num_bytes() {
            Ok(size) => {
                println!("sd card up: {} bytes", size);
                card.spi(|device| {
                    device
                        .bus_mut()
                        .apply_config(&bus_config(RUN_FREQUENCY))
                        .expect("invalid SD SPI configuration")
                });
                self.ready = true;

                Ok(())
            }
            Err(err) => {
                println!("no sd card: {:?}", err);
                self.reset();

                Err(SdError::NoCard)
            }
        }
    }

    /// Forgets the card, so the next operation starts it again.
    fn reset(&mut self) {
        if let Some(card) = &self.card {
            card.mark_card_uninit();
        }
        self.ready = false;
    }
}

fn bus_config(frequency: Rate) -> SpiConfig {
    SpiConfig::default()
        .with_frequency(frequency)
        .with_mode(Mode::_0)
}
