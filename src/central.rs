#![no_main]
#![no_std]

mod combined_processor;
mod keymap;
#[macro_use]
mod macros;
mod vial;

use defmt::info;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::flash::{Async, Flash};
use embassy_rp::gpio::{Input, Output};
use embassy_rp::peripherals::{PIO0, USB};
use embassy_rp::usb::{Driver, InterruptHandler};
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::futures::future::join;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::matrix::{Matrix, OffsetMatrixWrapper};
use rmk::split::SPLIT_MESSAGE_MAX_SIZE;
use rmk::split::central::run_peripheral_manager;
use rmk::split::rp::uart::{BufferedUart, UartInterruptHandler};
use rmk::{initialize_keymap_and_storage, run_devices, run_processor_chain, run_rmk};

use crate::combined_processor::CombinedProcessor;
use static_cell::StaticCell;
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => InterruptHandler<USB>;
    PIO0_IRQ_0 => UartInterruptHandler<PIO0>;
});

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("RMK start!");
    let p = embassy_rp::init(Default::default());

    // USB driver
    let driver = Driver::new(p.USB, Irqs);


    let (row_pins, col_pins) = config_matrix_pins_rp!(peripherals: p, input: [PIN_5, PIN_27, PIN_28, PIN_29, PIN_7], output: [PIN_1, PIN_26, PIN_22, PIN_21, PIN_23, PIN_20]);

        // Use internal flash to emulate eeprom
    let flash = Flash::<_, Async, FLASH_SIZE>::new(p.FLASH, p.DMA_CH0);

    let keyboard_device_config = DeviceConfig {
        vid: 0x4c4b,
        pid: 0x4643,
        manufacturer: "Good Great Grand Wonderful",
        product_name: "Crosses Wired",
        serial_number: "vial:f64c2b3c:000001",
    };
    let vial_config = VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (1, 1)]);
    let rmk_config = RmkConfig {
        device_config: keyboard_device_config,
        vial_config,
        ..Default::default()
    };

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
        // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut behavior_config = BehaviorConfig::default();
    let storage_config = StorageConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        &storage_config,
        &mut behavior_config,
        &mut per_key_config,
    )
    .await;

    // Initialize the matrix + keyboard
    let debouncer = DefaultDebouncer::new();

    let matrix = Matrix::<_, _, _, 5, 6, true>::new(row_pins, col_pins, debouncer);
    let mut matrix = OffsetMatrixWrapper::<_, _, _, 0, 0>(matrix);
    let mut keyboard = Keyboard::new(&keymap);

    // PMW3610 trackball — central (cursor), invert_x=true, invert_y=true, cpi=800
    let mut trackball0_device = {
        use embassy_rp::gpio::{Flex, Level, Output};
        use rmk::input_device::pmw3610::{BitBangSpiBus, Pmw3610Config, Pmw3610Device};

        let sck = Output::new(p.PIN_2, Level::High);
        let sdio = Flex::new(p.PIN_0);
        let cs = Output::new(p.PIN_6, Level::High);
        let motion = Some(embassy_rp::gpio::Input::new(
            p.PIN_3,
            embassy_rp::gpio::Pull::Up,
        ));

        let spi_bus = BitBangSpiBus::new(sck, sdio);

        let config = Pmw3610Config {
            res_cpi: 800,
            invert_x: true,
            invert_y: true,
            swap_xy: false,
            force_awake: false,
            smart_mode: true,
        };

        Pmw3610Device::new(spi_bus, cs, motion, config)
    };

    // Combined processor — handles cursor (X/Y) and scroll (H/V)
    let mut combined_processor = CombinedProcessor::new(&keymap, 20);

    // Run all tasks concurrently
    join(
        join(
            join(
                join(
                    run_devices!(
                        (matrix, trackball0_device) => EVENT_CHANNEL,
                    ),
                    keyboard.run(),
                ),
                run_processor_chain!(
                    EVENT_CHANNEL => [combined_processor],
                ),
            ),
            run_peripheral_manager::<5, 6, 0, 6, _>(0, uart_instance),
        ),
        run_rmk(&keymap, driver, &mut storage, rmk_config),
    )
    .await;
}
