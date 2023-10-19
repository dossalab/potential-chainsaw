use embedded_hal_async::delay;
enum MoveDirection{
    Straight,
    Back,
    Left,
    Right
}

trait DeviceControl{
    async fn go(&self, rotate_angle: usize, rotate_intensity: usize, move_dir: MoveDirection, move_intensity: usize, dur: impl delay::DelayUs);
}