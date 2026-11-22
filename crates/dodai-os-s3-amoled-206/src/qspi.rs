//! The CO5300's QSPI bus on top of esp-hal's DMA SPI.
//!
//! Register writes send opcode `0x02` and the register in the address phase, on one line. Pixels
//! go as opcode `0x32` with address `0x003C00` and the data on four lines, in chunks that fit the
//! DMA buffer, with chip-select held low for the whole frame region.

use alloc::vec;
use alloc::vec::Vec;
use dodai_os_drivers::co5300::QspiBus;
use esp_hal::Blocking;
use esp_hal::gpio::Output;
use esp_hal::spi::Error;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDmaBus};

/// Bytes per DMA transfer; the DMA buffer is this big.
pub const DMA_CHUNK: usize = 8000;

const OP_WRITE: u8 = 0x02;
const OP_WRITE_QUAD: u8 = 0x32;
const ADDR_RAMWR: u32 = 0x003C00;

/// The bus: SPI with DMA plus a manual chip-select.
pub struct Qspi<'d> {
    spi: SpiDmaBus<'d, Blocking>,
    cs: Output<'d>,
    scratch: Vec<u8>,
}

impl<'d> Qspi<'d> {
    /// Wraps an SPI bus and its chip-select pin.
    pub fn new(spi: SpiDmaBus<'d, Blocking>, cs: Output<'d>) -> Self {
        Self { spi, cs, scratch: vec![0; DMA_CHUNK] }
    }
}

impl QspiBus for Qspi<'_> {
    type Error = Error;

    fn write_reg(&mut self, reg: u8, data: &[u8]) -> Result<(), Error> {
        self.cs.set_low();
        let result = self.spi.half_duplex_write(
            DataMode::Single,
            Command::_8Bit(u16::from(OP_WRITE), DataMode::Single),
            Address::_24Bit(u32::from(reg) << 8, DataMode::Single),
            0,
            data,
        );
        self.cs.set_high();
        result
    }

    fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), Error> {
        self.cs.set_low();
        let mut first = true;
        let mut result = Ok(());
        for chunk in pixels.chunks(DMA_CHUNK / 2) {
            for (i, px) in chunk.iter().enumerate() {
                self.scratch[i * 2..i * 2 + 2].copy_from_slice(&px.to_be_bytes());
            }
            let data = &self.scratch[..chunk.len() * 2];
            result = if first {
                first = false;
                self.spi.half_duplex_write(
                    DataMode::Quad,
                    Command::_8Bit(u16::from(OP_WRITE_QUAD), DataMode::Single),
                    Address::_24Bit(ADDR_RAMWR, DataMode::Single),
                    0,
                    data,
                )
            } else {
                self.spi.half_duplex_write(DataMode::Quad, Command::None, Address::None, 0, data)
            };
            if result.is_err() {
                break;
            }
        }
        self.cs.set_high();
        result
    }
}
