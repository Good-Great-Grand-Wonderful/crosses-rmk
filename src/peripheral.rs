#![no_main]
#![no_std]

#[macro_use]
mod macros;
mod scroll_device;

use defmt::*;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Input, Output, Pull};
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::usb::InterruptHandler;
use rmk::channel::EVENT_CHANNEL;
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join;
use rmk::matrix::Matrix;
use rmk::run_devices;
use rmk::split::SPLIT_MESSAGE_MAX_SIZE;
use rmk::split::peripheral::run_rmk_split_peripheral;
use rmk::split::rp::uart::{BufferedUart, UartInterruptHandler};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

use crate::scroll_device::ScrollDevice;

// PIO UART interrupt binding for split serial
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    PIO0_IRQ_0 => UartInterruptHandler<PIO0>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    let p = embassy_rp::init(Default::default());

    // Matrix pins — peripheral: 5 rows (input) × 6 cols (output), col2row = true
    // Pin config
    let (row_pins, col_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_5, PIN_27, PIN_28, PIN_29, PIN_7], output: [PIN_1, PIN_26, PIN_22, PIN_21, PIN_23, PIN_20]);

    // PIO UART for split serial (half-duplex on PIN_4)
    static RX_BUF: StaticCell<[u8; SPLIT_MESSAGE_MAX_SIZE]> =
        StaticCell::new();
    let rx_buf = &mut RX_BUF.init([0; SPLIT_MESSAGE_MAX_SIZE])[..];

    let uart_instance = BufferedUart::new_half_duplex(
        p.PIO0,
        p.PIN_4,
        rx_buf,
        Irqs,
    );

    // Matrix
    let debouncer = DefaultDebouncer::new();
    let mut matrix = Matrix::<_, _, _, 5, 6, true>::new(row_pins, col_pins, debouncer);

    // PMW3610 trackball — peripheral (scroll), invert_x=true, invert_y=false, cpi=800
    // Wrapped in ScrollDevice to remap X→H and Y→V
    let mut scroll_device = {
        use embassy_rp::gpio::{Flex, Level, Output};
        use rmk::input_device::pmw3610::{BitBangSpiBus, Pmw3610Config, Pmw3610Device};

        let sck = Output::new(p.PIN_2, Level::High);
        let sdio = Flex::new(p.PIN_0);
        let cs = Output::new(p.PIN_6, Level::High);
        let motion = Some(Input::new(
            p.PIN_3,
            Pull::Up,
        ));

        let spi_bus = BitBangSpiBus::new(sck, sdio);

        let config = Pmw3610Config {
            res_cpi: 800,
            invert_x: true,
            invert_y: false,
            swap_xy: false,
            force_awake: false,
            smart_mode: true,
        };

        let pmw3610_device = Pmw3610Device::new(spi_bus, cs, motion, config);
        ScrollDevice::new(pmw3610_device)
    };


    // Run all tasks concurrently
    join(
        run_devices!(
            (matrix, scroll_device) => EVENT_CHANNEL,
        ),
        run_rmk_split_peripheral(uart_instance),
    )
    .await;
}
