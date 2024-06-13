#![no_main]
#![no_std]

//! NOTE: This example compiles on latest main branch, which may be different from released version

#[macro_use]
mod macros;
mod keymap;
mod st7789;
mod vial;

use core::cell::RefCell;

use crate::{
    keymap::{COL, NUM_LAYER, ROW},
    st7789::ST7789,
};
use defmt::*;
use defmt_rtt as _;
use embassy_embedded_hal::shared_bus::blocking::spi::SpiDevice;
use embassy_executor::Spawner;
use embassy_stm32::peripherals::DMA1_CH4;
use embassy_stm32::peripherals::SPI3;
use embassy_stm32::{
    bind_interrupts,
    flash::{Blocking, Flash},
    gpio::{AnyPin, Input, Level, Output, Speed},
    peripherals::USB_OTG_HS,
    spi::{self, Spi},
    time::{mhz, Hertz},
    usb_otg::{Driver, InterruptHandler},
    Config,
};
use embassy_stm32::{dma::NoDma, peripherals::DMA1_CH3};
use embassy_sync::blocking_mutex::{raw::NoopRawMutex, NoopMutex};
use embassy_time::Delay;
use embassy_time::Timer;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::Point,
    pixelcolor::{Rgb565, RgbColor},
    primitives::{Line, Primitive as _, PrimitiveStyle},
};
use embedded_graphics::{
    image::{Image, ImageRawLE},
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};
use embedded_graphics::{primitives::Rectangle, Drawable};
use panic_probe as _;
use rmk::{
    config::{RmkConfig, VialConfig},
    initialize_keyboard_and_run,
};
use static_cell::StaticCell;
use tinytga::Tga;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

// bind_interrupts!(struct Irqs {
//     OTG_HS => InterruptHandler<USB_OTG_HS>;
// });

static SPI_BUS: StaticCell<NoopMutex<RefCell<Spi<SPI3, DMA1_CH3, NoDma>>>> = StaticCell::new();
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    // RCC config
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hsi = Some(HSIPrescaler::DIV1);
        config.rcc.csi = true;
        // Needed for USB
        config.rcc.hsi48 = Some(Hsi48Config {
            sync_from_usb: true,
        });
        // External oscillator 25MHZ
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000),
            mode: HseMode::Oscillator,
        });
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL112,
            divp: Some(PllDiv::DIV2),
            divq: Some(PllDiv::DIV2),
            divr: Some(PllDiv::DIV2),
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV2;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV2;
        config.rcc.apb3_pre = APBPrescaler::DIV2;
        config.rcc.apb4_pre = APBPrescaler::DIV2;
        config.rcc.voltage_scale = VoltageScale::Scale0;
    }

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    let mut spi_config = spi::Config::default();
    spi_config.frequency = mhz(64);
    let spi = spi::Spi::new_txonly(p.SPI3, p.PB3, p.PB5, p.DMA1_CH3, NoDma, spi_config);
    let spi_bus = SPI_BUS.init(NoopMutex::new(RefCell::new(spi)));
    let cs = Output::new(p.PB9, Level::High, Speed::VeryHigh);
    let spi_device = SpiDevice::new(spi_bus, cs);
    let dc = Output::new(p.PB7, Level::High, Speed::VeryHigh);
    let mut display = ST7789::<_, _, 320, 172, 0, 34>::new(spi_device, dc);

    display.init(&mut Delay);
    display.clear(Rgb565::BLACK).unwrap();
    let raw_image_data = ImageRawLE::new(include_bytes!("./assets/ferris.raw"), 86);
    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::WHITE);
    let mut led = Output::new(p.PE3, Level::High, Speed::High);
    Text::new("Hello embedded_graphics!", Point::new(19, 150), style)
        .draw(&mut display)
        .unwrap();
    let diff = 2;
    let mut ferris = Image::new(&raw_image_data, Point::new(0, 40));
    loop {
        // led.toggle();
        let mut clear = Rectangle::new(
            Point {
                x: ferris.bounding_box().top_left.x,
                y: 40,
            },
            Size {
                width: diff as u32,
                height: 64,
            },
        );
        let f = if ferris.bounding_box().top_left.x + 86 >= 320 {
            clear.size.width = 86;
            ferris.translate_mut(Point::new(-234, 0))
        } else {
            ferris.translate_mut(Point::new(diff, 0))
        };

        f.draw(&mut display).unwrap();
        display.fill_solid(&clear, Rgb565::BLACK).unwrap();
    }

    // // Usb config
    // static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    // let mut usb_config = embassy_stm32::usb_otg::Config::default();
    // usb_config.vbus_detection = false;
    // let driver = Driver::new_fs(
    //     p.USB_OTG_HS,
    //     Irqs,
    //     p.PA12,
    //     p.PA11,
    //     &mut EP_OUT_BUFFER.init([0; 1024])[..],
    //     usb_config,
    // );

    // // Pin config
    // let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    // // Use internal flash to emulate eeprom
    // let f = Flash::new_blocking(p.FLASH);

    // let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    // let keyboard_config = RmkConfig {
    //     vial_config,
    //     ..Default::default()
    // };

    // // Start serving
    // initialize_keyboard_with_config_and_run::<
    //     Flash<'_, Blocking>,
    //     Driver<'_, USB_OTG_HS>,
    //     Input<'_, AnyPin>,
    //     Output<'_, AnyPin>,
    //     ROW,
    //     COL,
    //     NUM_LAYER,
    // >(
    //     driver,
    //     input_pins,
    //     output_pins,
    //     Some(f),
    //     crate::keymap::KEYMAP,
    //     keyboard_config,
    // )
    // .await;
}
    // // Start serving
    // initialize_keyboard_and_run::<
    //     Flash<'_, Blocking>,
    //     Driver<'_, USB_OTG_HS>,
    //     Input<'_, AnyPin>,
    //     Output<'_, AnyPin>,
    //     ROW,
    //     COL,
    //     NUM_LAYER,
    // >(
    //     driver,
    //     input_pins,
    //     output_pins,
    //     Some(f),
    //     crate::keymap::KEYMAP,
    //     keyboard_config,
    // )
    // .await;
}
