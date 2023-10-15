use crate::control::{ControlService, ControlServiceEvent};
use crate::power::{BatteryService, BatteryServiceEvent};

#[nrf_softdevice::gatt_server]
pub struct GattServer {
    bas: BatteryService,
    control: ControlService,
}
