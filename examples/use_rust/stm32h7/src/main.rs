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
use embassy_stm32::peripherals::DMA1_CH3;
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
use embassy_sync::blocking_mutex::{raw::NoopRawMutex, NoopMutex};
use panic_probe as _;
use rmk::{
    config::{RmkConfig, VialConfig},
    initialize_keyboard_with_config_and_run,
};
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    OTG_HS => InterruptHandler<USB_OTG_HS>;
});

static SPI_BUS: StaticCell<NoopMutex<RefCell<Spi<SPI3, DMA1_CH3, DMA1_CH4>>>> = StaticCell::new();
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
    spi_config.frequency = mhz(1);

    let spi = spi::Spi::new(
        p.SPI3, p.PB3, p.PB5, p.PB4, p.DMA1_CH3, p.DMA1_CH4, spi_config,
    );
    let spi_bus = NoopMutex::new(RefCell::new(spi));
    let spi_bus = SPI_BUS.init(spi_bus);
    let cs = Output::new(p.PB15, Level::High, Speed::High);
    let spi_device = SpiDevice::new(spi_bus, cs);

    let dc = Output::new(p.PB14, Level::High, Speed::High);
    let st7789 = ST7789::<_, _, 320, 172, 0, 0>::new(spi_device, dc);

    // Usb config
    static EP_OUT_BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();
    let mut usb_config = embassy_stm32::usb_otg::Config::default();
    usb_config.vbus_detection = false;
    let driver = Driver::new_fs(
        p.USB_OTG_HS,
        Irqs,
        p.PA12,
        p.PA11,
        &mut EP_OUT_BUFFER.init([0; 1024])[..],
        usb_config,
    );

    // Pin config
    let (input_pins, output_pins) = config_matrix_pins_stm32!(peripherals: p, input: [PD9, PD8, PB13, PB12], output: [PE13, PE14, PE15]);

    // Use internal flash to emulate eeprom
    let f = Flash::new_blocking(p.FLASH);

    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF);

    let keyboard_config = RmkConfig {
        vial_config,
        ..Default::default()
    };

    // Start serving
    initialize_keyboard_with_config_and_run::<
        Flash<'_, Blocking>,
        Driver<'_, USB_OTG_HS>,
        Input<'_, AnyPin>,
        Output<'_, AnyPin>,
        ROW,
        COL,
        NUM_LAYER,
    >(
        driver,
        input_pins,
        output_pins,
        Some(f),
        crate::keymap::KEYMAP,
        keyboard_config,
    )
    .await;
}
