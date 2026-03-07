use core::cell::RefCell;

use rmk::channel::KEYBOARD_REPORT_CHANNEL;
use rmk::event::{Axis, Event};
use rmk::hid::Report;
use rmk::input_device::{InputProcessor, ProcessResult};
use rmk::keymap::KeyMap;
use usbd_hid::descriptor::MouseReport;

/// Combined input processor that handles both cursor (X/Y axes) and scroll (H/V axes).
///
/// X/Y axes are forwarded directly as cursor movement.
/// H/V axes are accumulated and divided by `scroll_divisor` to produce scroll steps,
/// providing sensitivity control for the scroll wheel.
pub struct CombinedProcessor<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize = 0> {
    keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
    scroll_divisor: i16,
    h_accumulator: i16,
    v_accumulator: i16,
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    CombinedProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    pub fn new(
        keymap: &'a RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>>,
        scroll_divisor: i16,
    ) -> Self {
        Self {
            keymap,
            scroll_divisor,
            h_accumulator: 0,
            v_accumulator: 0,
        }
    }
}

impl<'a, const ROW: usize, const COL: usize, const NUM_LAYER: usize, const NUM_ENCODER: usize>
    InputProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
    for CombinedProcessor<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>
{
    async fn process(&mut self, event: Event) -> ProcessResult {
        match event {
            Event::Joystick(axes) => {
                let mut cursor_x: i8 = 0;
                let mut cursor_y: i8 = 0;
                let mut has_cursor = false;
                let mut has_scroll = false;

                for ae in &axes {
                    match ae.axis {
                        Axis::X => {
                            cursor_x = ae.value.clamp(-128, 127) as i8;
                            has_cursor = true;
                        }
                        Axis::Y => {
                            cursor_y = ae.value.clamp(-128, 127) as i8;
                            has_cursor = true;
                        }
                        Axis::H => {
                            self.h_accumulator += ae.value;
                            has_scroll = true;
                        }
                        Axis::V => {
                            self.v_accumulator += ae.value;
                            has_scroll = true;
                        }
                        _ => {}
                    }
                }

                // Send cursor movement if we have X/Y data
                if has_cursor && (cursor_x != 0 || cursor_y != 0) {
                    let report = MouseReport {
                        x: cursor_x,
                        y: cursor_y,
                        buttons: 0,
                        wheel: 0,
                        pan: 0,
                    };
                    KEYBOARD_REPORT_CHANNEL
                        .send(Report::MouseReport(report))
                        .await;
                }

                // Send scroll if accumulated enough
                if has_scroll {
                    let wheel = self.v_accumulator / self.scroll_divisor;
                    let pan = self.h_accumulator / self.scroll_divisor;

                    if wheel != 0 || pan != 0 {
                        self.v_accumulator -= wheel * self.scroll_divisor;
                        self.h_accumulator -= pan * self.scroll_divisor;

                        let report = MouseReport {
                            x: 0,
                            y: 0,
                            buttons: 0,
                            wheel: wheel.clamp(-128, 127) as i8,
                            pan: pan.clamp(-128, 127) as i8,
                        };
                        KEYBOARD_REPORT_CHANNEL
                            .send(Report::MouseReport(report))
                            .await;
                    }
                }

                ProcessResult::Stop
            }
            other => ProcessResult::Continue(other),
        }
    }

    fn get_keymap(&self) -> &RefCell<KeyMap<'a, ROW, COL, NUM_LAYER, NUM_ENCODER>> {
        self.keymap
    }
}
