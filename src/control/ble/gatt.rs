use super::control_srv::{ControlService, ControlServiceEvent};
use super::power::{BatteryService, BatteryServiceEvent};

#[nrf_softdevice::gatt_server]
pub struct GattServer {
    bas: BatteryService,
    control: ControlService,
}
