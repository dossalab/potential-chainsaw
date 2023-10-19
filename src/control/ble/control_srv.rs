#[nrf_softdevice::gatt_service(uuid = "9e7312e0-2354-11eb-9f10-fbc30a62cf38")]
pub struct ControlService {
    #[characteristic(uuid = "9e7312e0-2354-11eb-9f10-fbc30a63cf38", read, write)]
    foo: u16,
}
