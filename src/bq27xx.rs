use defmt::{debug, Format};
use embedded_hal_async::{delay, i2c};

const BQ27427_TWI_ADDRESS: u8 = 0x55;

/*
 * Chem id is related to the battery type, i.e charge / discharge curve
 */
#[derive(Format)]
pub enum ChemId {
    A4350,
    B4200,
    C4400,
    Unknown,
}

impl From<u16> for ChemId {
    fn from(code: u16) -> Self {
        match code {
            0x3230 => Self::A4350,
            0x1202 => Self::B4200,
            0x3142 => Self::C4400,
            _ => Self::Unknown,
        }
    }
}

/*
 * This code is tested on BQ27427, but I suspect that the interface
 * and commands are pretty similar across all TI bridges. YMMV
 */

#[derive(Format)]
pub enum DeviceType {
    BQ27421,
    BQ27426,
    BQ27427,
    Unknown,
}

impl From<u16> for DeviceType {
    fn from(code: u16) -> Self {
        match code {
            0x421 => Self::BQ27421,
            0x426 => Self::BQ27426,
            0x427 => Self::BQ27427,
            _ => Self::Unknown,
        }
    }
}

/*
 * This is a list of commands (i.e *registers*) supported by the gauge
 */
mod commands {
    #![allow(dead_code)]
    pub const CONTROL: u8 = 0x00;
    pub const TEMPERATURE: u8 = 0x02;
    pub const VOLTAGE: u8 = 0x04;
    pub const FLAGS: u8 = 0x06;
    pub const NOMINAL_AVAILABLE_CAPACITY: u8 = 0x08;
    pub const FULL_AVAILABLE_CAPACITY: u8 = 0x0A;
    pub const REMAINING_CAPACITY: u8 = 0x0C;
    pub const FULL_CHARGE_CAPACITY: u8 = 0x0E;
    pub const AVERAGE_CURRENT: u8 = 0x10;
    pub const AVERAGE_POWER: u8 = 0x18;
    pub const STATE_OF_CHARGE: u8 = 0x1C;
    pub const INTERNAL_TEMPERATURE: u8 = 0x1E;
    pub const STATE_OF_HEALTH: u8 = 0x20;
    pub const REMAINING_CAPACITY_UNFILTERED: u8 = 0x28;
    pub const REMAINING_CAPACITY_FILTERED: u8 = 0x2A;
    pub const FULL_CHARGE_CAPACITY_UNFILTERED: u8 = 0x2C;
    pub const FULL_CHARGE_CAPACITY_FILTERED: u8 = 0x2E;
    pub const STATE_OF_CHARGE_UNFILTERED: u8 = 0x30;

    // Extended, i.e direct memory access
    pub const DATA_CLASS: u8 = 0x3E;
    pub const DATA_BLOCK: u8 = 0x3F;
    pub const BLOCK_DATA_START: u8 = 0x40;
    pub const BLOCK_DATA_END: u8 = 0x5F;
    pub const BLOCK_DATA_CHECKSUM: u8 = 0x60;
    pub const BLOCK_DATA_CONTROL: u8 = 0x61;
}

/*
 * All memory locations in turn are divided into subclasses
 */
mod memory_subclass {
    #![allow(dead_code)]
    pub const SAFETY: u8 = 2;
    pub const CHARGE_TERMINATION: u8 = 36;
    pub const DISCHARGE: u8 = 49;
    pub const REGISTERS: u8 = 64;
    pub const IT_CFG: u8 = 80;
    pub const CURRENT_THRESHOLDS: u8 = 81;
    pub const STATE: u8 = 82;
    pub const RA0_RAM: u8 = 89;
    pub const CHEM_DATA: u8 = 109;
    pub const DATA: u8 = 104;
    pub const CC_CAL: u8 = 105;
    pub const CURRENT: u8 = 107;
    pub const CODES: u8 = 112;
}

/*
 * Issuing a Control() command requires a subsequent 2-byte subcommand.
 * Additional bytes specify the particular control function desired
 */
mod control_subcommands {
    #![allow(dead_code)]
    pub const CONTROL_STATUS: u16 = 0x0000;
    pub const DEVICE_TYPE: u16 = 0x0001;
    pub const FW_VERSION: u16 = 0x0002;
    pub const DM_CODE: u16 = 0x0004;
    pub const PREV_MACWRITE: u16 = 0x0007;
    pub const CHEM_ID: u16 = 0x0008;
    pub const BAT_INSERT: u16 = 0x000C;
    pub const BAT_REMOVE: u16 = 0x000D;
    pub const SET_CFGUPDATE: u16 = 0x0013;
    pub const SMOOTH_SYNC: u16 = 0x0019;
    pub const SHUTDOWN_ENABLE: u16 = 0x001B;
    pub const SHUTDOWN: u16 = 0x001C;
    pub const SEALED: u16 = 0x0020;
    pub const PULSE_SOC_INT: u16 = 0x0023;
    pub const CHEM_A: u16 = 0x0030;
    pub const CHEM_B: u16 = 0x0031;
    pub const CHEM_C: u16 = 0x0032;
    pub const RESET: u16 = 0x0041;
    pub const SOFT_RESET: u16 = 0x0042;
}

/*
 * Contents of the flags register, returned by the "Flags" command
 */
mod flags {
    #![allow(dead_code)]
    pub const CFGUPMODE: u16 = 1 << 4;
    // TODO add others...
}

/*
 * There are honestly not that many errors possible, we either have
 * the device on the bus or we die
 */

pub enum DeviceError<E> {
    I2CError(E),
    PollTimeout,
}

impl<E> From<E> for DeviceError<E> {
    fn from(e: E) -> Self {
        Self::I2CError(e)
    }
}

#[cfg(feature = "defmt")]
impl<I> defmt::Format for DeviceError<I> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "error");
    }
}

pub struct Bq27xx<I, D> {
    i2c: I,
    delay: D,
    addr: u8,
}

impl<I, D, E> Bq27xx<I, D>
where
    D: delay::DelayUs,
    I: i2c::I2c<Error = E>,
{
    /*
     * The protocol is wacky - see the datasheet for the details. Long story short
     * the chip does not like long transactions so writing the command and getting the
     * response are 2 different operations.
     */
    async fn read_control(&mut self, subcommand: u16) -> Result<u16, DeviceError<E>> {
        let mut response = [0, 0];
        let request = [commands::CONTROL, subcommand as u8, (subcommand >> 8) as u8];

        self.i2c.write(self.addr, &request).await?;
        self.i2c
            .write_read(self.addr, &[commands::CONTROL], &mut response)
            .await?;

        Ok(u16::from_le_bytes(response))
    }

    async fn write_control(&mut self, subcommand: u16) -> Result<(), DeviceError<E>> {
        let request = [commands::CONTROL, subcommand as u8, (subcommand >> 8) as u8];
        self.i2c.write(self.addr, &request).await?;
        Ok(())
    }

    async fn read_command(&mut self, command: u8) -> Result<u16, DeviceError<E>> {
        let mut response = [0, 0];

        self.i2c
            .write_read(self.addr, &[command], &mut response)
            .await?;

        Ok(u16::from_le_bytes(response))
    }

    async fn write_command(&mut self, command: u8, data: u8) -> Result<(), DeviceError<E>> {
        let request = [command, data];
        self.i2c.write(self.addr, &request).await?;
        Ok(())
    }

    async fn unseal(&mut self) -> Result<(), DeviceError<E>> {
        debug!("unsealing the chip");

        // FIXME
        Ok(())
    }

    async fn seal(&mut self) -> Result<(), DeviceError<E>> {
        debug!("sealing the chip");

        // FIXME
        Ok(())
    }

    pub async fn get_flags(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_command(commands::FLAGS).await
    }

    /*
     * TODO: this uses embassy time primitives, which is not really portable..
     */
    async fn wait_flags(&mut self, mask: u16) -> Result<(), DeviceError<E>> {
        const FLAG_POLL_RETRIES: u32 = 10;

        for _ in 0..FLAG_POLL_RETRIES {
            self.delay.delay_ms(500).await;

            let flags = self.get_flags().await?;

            debug!("read flag register - 0b{:b}", flags);

            if (flags & mask) != 0 {
                return Ok(());
            }
        }

        Err(DeviceError::PollTimeout)
    }

    async fn mode_cfgupdate(&mut self) -> Result<(), DeviceError<E>> {
        debug!("entering cfgupdate mode...");

        self.write_control(control_subcommands::SET_CFGUPDATE)
            .await?;
        self.wait_flags(flags::CFGUPMODE).await?;

        debug!("cfgupdate mode entered!");

        Ok(())
    }

    /*
     * Besides regular commands and control commands there is also direct
     * memory access using datablocks - some parameters are only
     * available through that interface. Bravo TI!
     */
    async fn read_mem_simple(&mut self, class: u8, offset: u8) -> Result<u16, DeviceError<E>> {
        let mut checksum = [0];
        const BLOCKSIZE: u8 = 32;

        self.unseal().await?;
        self.mode_cfgupdate().await?;

        debug!("reading memory at 0x{:x}...", offset);

        /* Writing 0 to this register allows direct memory access, see datasheet */
        self.write_command(commands::BLOCK_DATA_CONTROL, 0).await?;

        self.write_command(commands::DATA_CLASS, class).await?;
        self.write_command(commands::DATA_BLOCK, offset / BLOCKSIZE)
            .await?;

        self.i2c
            .write_read(self.addr, &[commands::BLOCK_DATA_CHECKSUM], &mut checksum)
            .await?;

        debug!("checksum is 0x{:x}", checksum[0]);

        let response = self
            .read_command(commands::BLOCK_DATA_START + (offset % BLOCKSIZE))
            .await?;

        self.soft_reset().await?;
        Ok(response)
    }

    pub async fn set_chem_id(&mut self, id: ChemId) -> Result<(), DeviceError<E>> {
        self.unseal().await?;
        self.mode_cfgupdate().await?;

        let subcommand = match id {
            ChemId::A4350 => control_subcommands::CHEM_A,
            ChemId::B4200 => control_subcommands::CHEM_B,
            ChemId::C4400 => control_subcommands::CHEM_C,
            ChemId::Unknown => panic!("cannot set unknown chem id!"),
        };

        debug!("writing chem id {}...", id);

        self.write_control(subcommand).await?;
        self.soft_reset().await
    }

    pub async fn get_chem_id(&mut self) -> Result<ChemId, DeviceError<E>> {
        let response = self.read_control(control_subcommands::CHEM_ID).await?;
        debug!("chem id is 0x{:x}", response);
        Ok(ChemId::from(response))
    }

    pub async fn get_capacity(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_mem_simple(memory_subclass::STATE, 6).await
    }

    pub async fn reset(&mut self) -> Result<(), DeviceError<E>> {
        debug!("performing hard reset...");
        self.write_control(control_subcommands::RESET).await
    }

    pub async fn soft_reset(&mut self) -> Result<(), DeviceError<E>> {
        debug!("performing soft reset...");
        self.write_control(control_subcommands::SOFT_RESET).await
    }

    pub async fn state_of_charge(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_command(commands::STATE_OF_CHARGE).await
    }

    pub async fn voltage(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_command(commands::VOLTAGE).await
    }

    pub async fn temperature(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_command(commands::TEMPERATURE).await
    }

    pub async fn fw_version(&mut self) -> Result<u16, DeviceError<E>> {
        self.read_control(control_subcommands::FW_VERSION).await
    }

    pub async fn probe(&mut self) -> Result<DeviceType, DeviceError<E>> {
        let response = self.read_control(control_subcommands::DEVICE_TYPE).await?;
        debug!("device type is: {}", response);
        Ok(DeviceType::from(response))
    }

    pub fn new_with_address(i2c: I, delay: D, addr: u8) -> Self {
        Self { i2c, addr, delay }
    }

    pub fn new(i2c: I, delay: D) -> Self {
        Self::new_with_address(i2c, delay, BQ27427_TWI_ADDRESS)
    }
}
