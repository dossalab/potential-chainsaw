use defmt::{info, unwrap};
use embassy_executor::Spawner;
use embassy_nrf::gpio;

#[nrf_softdevice::gatt_service(uuid = "180f")]
pub struct BatteryService {
    #[characteristic(uuid = "2a19", read, notify)]
    battery_level: u8,
}

#[embassy_executor::task]
async fn gauge_watcher(pin: gpio::AnyPin) {
    let mut interrupt_input = gpio::Input::new(pin, gpio::Pull::Up);

    loop {
        interrupt_input.wait_for_low().await;
        info!("gauge interrupt detected");
    }
}

pub async fn init(spawner: Spawner) {
    info!("initializing power module...");

    // unwrap!(spawner.spawn(gauge_watcher()));
    // let mut gauge = bq27xx::Bq27xx::new(&mut i2c);

    // unwrap!(gauge.set_chem_id(bq27xx::ChemId::B4200).await);

    // info!("selected chem id is {}", unwrap!(gauge.get_chem_id().await));
    // info!("capacity is {}", unwrap!(gauge.get_capacity().await));
}
