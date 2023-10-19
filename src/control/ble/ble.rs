use defmt::info;
use defmt::unwrap;

use embassy_executor::Spawner;
use nrf_softdevice::ble;
use nrf_softdevice::ble::peripheral as blep;
use nrf_softdevice::raw as nrf_defines;
use nrf_softdevice::Softdevice;

use super::gatt::{GattServer, GattServerEvent};

const ADVERTISEMENT_INTERVAL: u32 = 1500;

async fn advertise(softdevice: &Softdevice) -> Result<ble::Connection, blep::AdvertiseError> {
    /* Weird that we had to do it manually, why is it so inconvenient? */
    #[rustfmt::skip]
    let payload = &[
        2, nrf_defines::BLE_GAP_AD_TYPE_FLAGS as u8, nrf_defines::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x0a, nrf_defines::BLE_GAP_AD_TYPE_COMPLETE_LOCAL_NAME as u8, b'H', b'e', b'l', b'l', b'o', b'R', b'u', b's', b't',
    ];

    #[rustfmt::skip]
    let scandata = &[
        0x03, 0x03, 0x09, 0x18,
    ];

    let packet = blep::ConnectableAdvertisement::ScannableUndirected {
        adv_data: payload,
        scan_data: scandata,
    };

    let config = blep::Config {
        interval: ADVERTISEMENT_INTERVAL,
        ..blep::Config::default()
    };

    return blep::advertise_connectable(softdevice, packet, &config).await;
}

async fn init_softdevice() -> &'static mut Softdevice {
    let config = nrf_softdevice::Config {
        /* Those values are for internal LF crystal */
        clock: Some(nrf_defines::nrf_clock_lf_cfg_t {
            source: nrf_defines::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: nrf_defines::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        ..Default::default()
    };

    /* This does not return errors, only panics if an error occurs */
    return Softdevice::enable(&config);
}

#[embassy_executor::task]
async fn softdevice_run(softdevice: &'static Softdevice) -> ! {
    softdevice.run().await
}

#[embassy_executor::task]
async fn handle_connections(softdevice: &'static Softdevice, server: GattServer) -> ! {
    loop {
        let connection = unwrap!(advertise(softdevice).await);
        let _ = ble::gatt_server::run(&connection, &server, |e| match e {
            GattServerEvent::Bas(_e) => {}
            _ => {}
        })
        .await;
    }
}

pub async fn init(spawner: Spawner) {
    info!("initializing SoftDevice...");

    let softdevice = init_softdevice().await;
    let server = unwrap!(GattServer::new(softdevice));

    unwrap!(spawner.spawn(softdevice_run(softdevice)));
    unwrap!(spawner.spawn(handle_connections(softdevice, server)));
}
