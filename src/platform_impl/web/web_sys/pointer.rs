use std::cell::Cell;
use std::rc::Rc;

use web_sys::PointerEvent;

use super::canvas::Common;
use super::event;
use super::event_handle::EventListenerHandle;
use crate::dpi::PhysicalPosition;
use crate::event::{ButtonSource, DeviceId, ElementState, Force, PointerKind, PointerSource};
use crate::keyboard::ModifiersState;
use crate::platform_impl::web::event::mkdid;

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_press: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_pointer_release: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
    on_touch_cancel: Option<EventListenerHandle<dyn FnMut(PointerEvent)>>,
}

impl PointerHandler {
    pub fn new() -> Self {
        Self {
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_press: None,
            on_pointer_release: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_pointer_leave<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_leave =
            Some(canvas_common.add_event("pointerout", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = mkdid(pointer_id);
                let position =
                    event::mouse_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_type(&event, pointer_id);
                handler(modifiers, device_id, event.is_primary(), position, kind);
            }));
    }

    pub fn on_pointer_enter<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, PointerKind),
    {
        let window = canvas_common.window.clone();
        self.on_cursor_enter =
            Some(canvas_common.add_event("pointerover", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let device_id = mkdid(pointer_id);
                let position =
                    event::mouse_position(&event).to_physical(super::scale_factor(&window));
                let kind = event::pointer_type(&event, pointer_id);
                handler(modifiers, device_id, event.is_primary(), position, kind);
            }));
    }

    pub fn on_pointer_release<C>(&mut self, canvas_common: &Common, mut handler: C)
    where
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
    {
        let window = canvas_common.window.clone();
        self.on_pointer_release =
            Some(canvas_common.add_event("pointerup", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let kind = event::pointer_type(&event, pointer_id);

                let button = event::mouse_button(&event).expect("no mouse button pressed");

                let source = match kind {
                    PointerKind::Mouse => ButtonSource::Mouse(button),
                    PointerKind::Touch(finger_id) => ButtonSource::Touch {
                        finger_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                    },
                    PointerKind::Pen(pointer_id) => ButtonSource::Pen {
                        pen_id: pointer_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                        twist: None,
                        tilt_x: None,
                        tilt_y: None,
                        tilt_altitude: None,
                        tilt_azimuth: None,
                        button_state: None,
                        state_info: Default::default(),
                    },
                    _ => ButtonSource::Unknown(button.to_id()),
                };

                handler(
                    modifiers,
                    mkdid(pointer_id),
                    event.is_primary(),
                    event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                    source,
                )
            }));
    }

    pub fn on_pointer_press<C>(
        &mut self,
        canvas_common: &Common,
        mut handler: C,
        prevent_default: Rc<Cell<bool>>,
    ) where
        C: 'static
            + FnMut(ModifiersState, Option<DeviceId>, bool, PhysicalPosition<f64>, ButtonSource),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_pointer_press =
            Some(canvas_common.add_event("pointerdown", move |event: PointerEvent| {
                if prevent_default.get() {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }

                let modifiers = event::mouse_modifiers(&event);
                let pointer_id = event.pointer_id();
                let kind = event::pointer_type(&event, pointer_id);
                let button = event::mouse_button(&event).expect("no mouse button pressed");

                let source = match kind {
                    PointerKind::Mouse => {
                        // Error is swallowed here since the error would occur every time the
                        // mouse is clicked when the cursor is
                        // grabbed, and there is probably not a
                        // situation where this could fail, that we
                        // care if it fails.
                        let _e = canvas.set_pointer_capture(pointer_id);

                        ButtonSource::Mouse(button)
                    },
                    PointerKind::Touch(finger_id) => ButtonSource::Touch {
                        finger_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                    },
                    PointerKind::Pen(pointer_id) => ButtonSource::Pen {
                        pen_id: pointer_id,
                        force: Some(Force::Normalized(event.pressure().into())),
                        twist: None,
                        tilt_x: None,
                        tilt_y: None,
                        tilt_altitude: None,
                        tilt_azimuth: None,
                        button_state: None,
                        state_info: Default::default(),
                    },
                    _ => ButtonSource::Unknown(button.to_id()),
                };

                handler(
                    modifiers,
                    mkdid(pointer_id),
                    event.is_primary(),
                    event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                    source,
                )
            }));
    }

    pub fn on_pointer_move<C, B>(
        &mut self,
        canvas_common: &Common,
        mut cursor_handler: C,
        mut button_handler: B,
        prevent_default: Rc<Cell<bool>>,
    ) where
        C: 'static
            + FnMut(
                Option<DeviceId>,
                &mut dyn Iterator<
                    Item = (ModifiersState, bool, PhysicalPosition<f64>, PointerSource),
                >,
            ),
        B: 'static
            + FnMut(
                ModifiersState,
                Option<DeviceId>,
                bool,
                PhysicalPosition<f64>,
                ElementState,
                ButtonSource,
            ),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        self.on_cursor_move =
            Some(canvas_common.add_event("pointermove", move |event: PointerEvent| {
                let pointer_id = event.pointer_id();
                let device_id = mkdid(pointer_id);
                let kind = event::pointer_type(&event, pointer_id);
                let primary = event.is_primary();

                // chorded button event
                if let Some(button) = event::mouse_button(&event) {
                    if prevent_default.get() {
                        // prevent text selection
                        event.prevent_default();
                        // but still focus element
                        let _ = canvas.focus();
                    }

                    let state = if event::mouse_buttons(&event).contains(button.into()) {
                        ElementState::Pressed
                    } else {
                        ElementState::Released
                    };

                    let button = match kind {
                        PointerKind::Mouse => ButtonSource::Mouse(button),
                        PointerKind::Touch(finger_id) => {
                            let button_id = button.to_id();

                            if button_id != 1 {
                                tracing::error!("unexpected touch button id: {button_id}");
                            }

                            ButtonSource::Touch {
                                finger_id,
                                force: Some(Force::Normalized(event.pressure().into())),
                            }
                        },
                        _ => ButtonSource::Unknown(button.to_id()),
                    };

                    button_handler(
                        event::mouse_modifiers(&event),
                        device_id,
                        primary,
                        event::mouse_position(&event).to_physical(super::scale_factor(&window)),
                        state,
                        button,
                    );

                    return;
                }

                // pointer move event
                let scale = super::scale_factor(&window);

                cursor_handler(
                    device_id,
                    &mut event::pointer_move_event(event).map(|event| {
                        (
                            event::mouse_modifiers(&event),
                            event.is_primary(),
                            event::mouse_position(&event).to_physical(scale),
                            match kind {
                                PointerKind::Mouse => PointerSource::Mouse,
                                PointerKind::Touch(finger_id) => PointerSource::Touch {
                                    finger_id,
                                    force: Some(Force::Normalized(event.pressure().into())),
                                },
                                PointerKind::Pen(pointer_id) => {
                                    // Workaround for https://github.com/rustwasm/wasm-bindgen/issues/4501
                                    use js_sys::Reflect;
                                    use wasm_bindgen::JsValue;
                                    let js_event: &JsValue = event.as_ref();
                                    let tilt_x =
                                        Reflect::get(js_event, &JsValue::from_str("tiltX"))
                                            .ok()
                                            .and_then(|v| v.as_f64());
                                    let tilt_y =
                                        Reflect::get(js_event, &JsValue::from_str("tiltY"))
                                            .ok()
                                            .and_then(|v| v.as_f64());
                                    let tilt_altitude =
                                        Reflect::get(js_event, &JsValue::from_str("altitudeAngle"))
                                            .ok()
                                            .and_then(|v| v.as_f64());
                                    let tilt_azimuth =
                                        Reflect::get(js_event, &JsValue::from_str("azimuthAngle"))
                                            .ok()
                                            .and_then(|v| v.as_f64());

                                    web_sys::console::log_1(
                                        &format!(
                                            "TILT INFO: {:?} {:?} {:?} {:?}",
                                            tilt_x, tilt_y, tilt_altitude, tilt_azimuth
                                        )
                                        .into(),
                                    );

                                    PointerSource::Pen {
                                        pen_id: pointer_id,
                                        force: Some(Force::Normalized(event.pressure().into())),
                                        twist: None,
                                        tilt_x: tilt_x.map(|x| x as f32),
                                        tilt_y: tilt_y.map(|x| x as f32),
                                        tilt_altitude: tilt_altitude.map(|x| x.to_degrees() as f32 - 90.0),
                                        tilt_azimuth: tilt_azimuth.map(|x| (x.to_degrees() as f32 + 90.0) % 360.0),
                                        button_state: None,             // TODO
                                        state_info: Default::default(), // TODO
                                    }
                                },
                                _ => PointerSource::Unknown,
                            },
                        )
                    }),
                );
            }));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_leave = None;
        self.on_cursor_enter = None;
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_cancel = None;
    }
}
