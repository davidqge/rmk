use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    pixelcolor::{raw::ToBytes as _, Rgb565},
};
use embedded_hal::{delay::DelayNs, digital::OutputPin, spi::SpiDevice};
pub struct ST7789<
    Spi: SpiDevice,
    Output: OutputPin,
    const WIDTH: u16,
    const HEIGHT: u16,
    const OFFSETX: u16,
    const OFFSETY: u16,
> {
    spi: Spi,
    dc: Output,
    // _marker: core::marker::PhantomData<(OFFSETX, OFFSETY)>,
}

///
/// Display orientation.
///
#[repr(u8)]
#[derive(Copy, Clone)]
pub enum Orientation {
    Portrait = 0b0000_0000,         // no inverting
    Landscape = 0b0110_0000,        // invert column and page/column order
    PortraitSwapped = 0b1100_0000,  // invert page and column order
    LandscapeSwapped = 0b1010_0000, // invert page and page/column order
}

impl Default for Orientation {
    fn default() -> Self {
        Self::Portrait
    }
}

impl<
        Spi: SpiDevice,
        Output: OutputPin,
        const WIDTH: u16,
        const HEIGHT: u16,
        const OFFSETX: u16,
        const OFFSETY: u16,
    > ST7789<Spi, Output, WIDTH, HEIGHT, OFFSETX, OFFSETY>
{
    pub fn new(spi: Spi, dc: Output) -> Self {
        Self { spi, dc }
    }

    pub fn init(&mut self, delay_source: &mut impl DelayNs) {
        delay_source.delay_us(10_000);
        // self.hard_reset(delay_source)?;
        // if let Some(bl) = self.bl.as_mut() {
        //     bl.set_low().map_err(Error::Pin)?;
        //     delay_source.delay_us(10_000);
        //     bl.set_high().map_err(Error::Pin)?;
        // }

        self.send_command(Instruction::SWRESET); // reset display
        delay_source.delay_us(150_000);
        self.send_command(Instruction::SLPOUT); // turn off sleep
        delay_source.delay_us(10_000);
        self.send_command(Instruction::INVOFF); // turn off invert
        self.send_command_data(Instruction::VSCRDER, &[0u8, 0u8, 0x14u8, 0u8, 0u8, 0u8]); // vertical scroll definition
        self.send_command_data(Instruction::MADCTL, &[Orientation::Landscape as u8]); // left -> right, bottom -> top RGB
        self.send_command_data(Instruction::COLMOD, &[0b0101_0101]); // 16bit 65k colors
        self.send_command(Instruction::INVON); // hack?
        delay_source.delay_us(10_000);
        self.send_command(Instruction::NORON); // turn on display
        delay_source.delay_us(10_000);
        self.send_command(Instruction::DISPON); // turn on display
        delay_source.delay_us(10_000);
    }

    // ///
    // /// Returns currently set orientation
    // ///
    // pub fn orientation(&self) -> Orientation {
    //     self.orientation
    // }

    // ///
    // /// Sets display orientation
    // ///
    // pub fn set_orientation(&mut self, orientation: Orientation) -> Result<(), Error<PinE>> {
    //     self.write_command(Instruction::MADCTL)?;
    //     self.write_data(&[orientation as u8])?;
    //     self.orientation = orientation;
    //     Ok(())
    // }

    #[inline]
    fn set_update_window(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let ox = OFFSETX + x;
        let oy = OFFSETY + y;

        self.send_command_data(
            Instruction::CASET,
            &[
                (ox >> 8) as u8,
                (ox & 0xFF) as u8,
                ((ox + w - 1) >> 8) as u8,
                ((ox + w - 1) & 0xFF) as u8,
            ],
        );

        self.send_command_data(
            Instruction::RASET,
            &[
                (oy >> 8) as u8,
                (oy & 0xFF) as u8,
                ((oy + h - 1) >> 8) as u8,
                ((oy + h - 1) & 0xFF) as u8,
            ],
        );
    }

    pub fn write_raw_pixel(&mut self, x: u16, y: u16, data: &[u8]) {
        self.set_update_window(x, y, 1, 1);

        self.send_command_data(Instruction::RAMWR, data);
    }

    fn send_command(&mut self, cmd: Instruction) {
        self.dc.set_low().unwrap();
        self.spi.write(&[cmd as u8]).unwrap();
    }

    fn send_data(&mut self, data: &[u8]) {
        self.dc.set_high().unwrap();
        self.spi.write(data).unwrap();
    }

    fn send_command_data(&mut self, cmd: Instruction, data: &[u8]) {
        self.send_command(cmd);
        self.send_data(data);
    }
}

impl<
        Spi: SpiDevice,
        Output: OutputPin,
        const WIDTH: u16,
        const HEIGHT: u16,
        const OFFSETX: u16,
        const OFFSETY: u16,
    > OriginDimensions for ST7789<Spi, Output, WIDTH, HEIGHT, OFFSETX, OFFSETY>
{
    fn size(&self) -> Size {
        Size::new(WIDTH as _, HEIGHT as _)
    }
}

impl<
        Spi: SpiDevice,
        Output: OutputPin,
        const WIDTH: u16,
        const HEIGHT: u16,
        const OFFSETX: u16,
        const OFFSETY: u16,
    > DrawTarget for ST7789<Spi, Output, WIDTH, HEIGHT, OFFSETX, OFFSETY>
{
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::prelude::Pixel<Self::Color>>,
    {
        for pixel in pixels {
            let x = pixel.0.x as u16;
            let y = pixel.0.y as u16;
            let color = pixel.1;

            self.write_raw_pixel(x, y, color.to_be_bytes().as_ref());
        }
        Ok(())
    }
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.set_update_window(0, 0, WIDTH, HEIGHT);

        self.send_command(Instruction::RAMWR);
        for _ in 0..((WIDTH as u16) * (HEIGHT as u16)) {
            self.send_data(color.to_be_bytes().as_ref());
        }
        Ok(())
    }

    fn fill_contiguous<I>(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        colors: I,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.set_update_window(
            area.top_left.x as u16,
            area.top_left.y as u16,
            area.size.width as u16,
            area.size.height as u16,
        );

        self.send_command(Instruction::RAMWR);
        for color in colors {
            self.send_data(color.to_be_bytes().as_ref());
        }
        Ok(())
    }
    fn fill_solid(
        &mut self,
        area: &embedded_graphics::primitives::Rectangle,
        color: Self::Color,
    ) -> Result<(), Self::Error> {
        self.set_update_window(
            area.top_left.x as u16,
            area.top_left.y as u16,
            area.size.width as u16,
            area.size.height as u16,
        );

        self.send_command(Instruction::RAMWR);
        for _ in 0..(area.size.width * area.size.height) {
            self.send_data(color.to_be_bytes().as_ref());
        }
        Ok(())
    }
}

/// ST7789 instructions.
#[repr(u8)]
pub enum Instruction {
    NOP = 0x00,
    SWRESET = 0x01,
    RDDID = 0x04,
    RDDST = 0x09,
    SLPIN = 0x10,
    SLPOUT = 0x11,
    PTLON = 0x12,
    NORON = 0x13,
    INVOFF = 0x20,
    INVON = 0x21,
    DISPOFF = 0x28,
    DISPON = 0x29,
    CASET = 0x2A,
    RASET = 0x2B,
    RAMWR = 0x2C,
    RAMRD = 0x2E,
    PTLAR = 0x30,
    VSCRDER = 0x33,
    TEOFF = 0x34,
    TEON = 0x35,
    MADCTL = 0x36,
    VSCAD = 0x37,
    COLMOD = 0x3A,
    VCMOFSET = 0xC5,
}
