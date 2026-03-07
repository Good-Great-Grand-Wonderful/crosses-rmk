use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::spi::SpiBus;
use rmk::event::{Axis, AxisEvent, Event};
use rmk::input_device::pmw3610::Pmw3610Device;
use rmk::input_device::InputDevice;

/// Wraps a Pmw3610Device, remapping X→H and Y→V so that the peripheral
/// trackball produces scroll events instead of cursor events.
pub struct ScrollDevice<SPI: SpiBus, CS: OutputPin, MOTION: InputPin> {
    inner: Pmw3610Device<SPI, CS, MOTION>,
}

impl<SPI: SpiBus, CS: OutputPin, MOTION: InputPin> ScrollDevice<SPI, CS, MOTION> {
    pub fn new(inner: Pmw3610Device<SPI, CS, MOTION>) -> Self {
        Self { inner }
    }
}

impl<SPI: SpiBus, CS: OutputPin, MOTION: InputPin> InputDevice for ScrollDevice<SPI, CS, MOTION> {
    async fn read_event(&mut self) -> Event {
        let event = self.inner.read_event().await;
        match event {
            Event::Joystick(axes) => {
                let remapped = axes.map(|ae| match ae.axis {
                    Axis::X => AxisEvent { axis: Axis::H, ..ae },
                    Axis::Y => AxisEvent { axis: Axis::V, ..ae },
                    _ => ae,
                });
                defmt::info!("ScrollDevice remapped: {:?}", remapped);
                Event::Joystick(remapped)
            }
            other => other,
        }
    }
}
