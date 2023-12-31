#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use bq27xxx::{Bq27xx, ChemId};
use defmt::{info, unwrap};
use git_version::git_version;

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    gpio::{self, Pin},
    interrupt, peripherals, twim, Peripherals,
};
use embassy_time::{Duration, Timer};

mod ble;
mod common;
mod control;
mod gatt;
mod power;

bind_interrupts!(struct Irqs {
    SPIM0_SPIS0_TWIM0_TWIS0_SPI0_TWI0 => twim::InterruptHandler<peripherals::TWISPI0>;
});

#[embassy_executor::task]
async fn blinky(pin: gpio::AnyPin) {
    let interval = Duration::from_millis(1000);
    let mut led = gpio::Output::new(pin, gpio::Level::Low, gpio::OutputDrive::Standard);

    loop {
        led.set_high();
        Timer::after(interval).await;
        led.set_low();
        Timer::after(interval).await;
    }
}

fn embassy_init() -> Peripherals {
    let mut config = embassy_nrf::config::Config::default();

    /*
     * Softdevice implicitly utilizes the highest-level interrupt priority
     * We have to move all other interrupts to lower priority, unless
     * random issues and asserts from the Softdevice may (and will) occur
     */
    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;

    return embassy_nrf::init(config);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_init();

    info!("Syma S107g mod ({}) is starting. Hello!", git_version!());

    ble::init(spawner).await;
    power::init(spawner).await;

    unwrap!(spawner.spawn(blinky(p.P0_00.degrade())));

    let config = twim::Config::default();
    let mut i2c = twim::Twim::new(p.TWISPI0, Irqs, p.P0_07, p.P0_08, config);

    let mut gauge = Bq27xx::new(&mut i2c, embassy_time::Delay, 0x55);

    info!("device is {}", unwrap!(gauge.device_type().await));
    unwrap!(gauge.write_chem_id(ChemId::B4200).await);

    loop {
        Timer::after_secs(3).await;

        info!("SOC is {}%", unwrap!(gauge.state_of_charge().await));
        info!("voltage is {}mv", unwrap!(gauge.voltage().await));
        info!("current is {}mA", unwrap!(gauge.average_current().await));
        info!("flags are [{}]", unwrap!(gauge.get_flags().await));
    }
}
