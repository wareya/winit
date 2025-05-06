//! Tablet tester.

use std::error::Error;

use font8x8::legacy::BASIC_LEGACY;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesExtWeb;
use winit::window::{Window, WindowAttributes, WindowId};

fn draw_char(frame: &mut [u32], width: usize, x: usize, y: usize, ch: char) {
    let glyph = BASIC_LEGACY.get(ch as usize).unwrap_or(&BASIC_LEGACY[' ' as usize]);
    for (row, byte) in glyph.iter().enumerate() {
        let ypart = (y + row) * width;
        for col in 0..8 {
            if byte & (1 << col) != 0 {
                let i = ypart + (x + col);
                if i < frame.len() {
                    frame[i] = 0xffffffff;
                }
            }
        }
    }
}

fn draw_text(frame: &mut [u32], width: usize, mut x: usize, mut y: usize, text: &str) {
    let x_init = x;
    for ch in text.chars() {
        if ch == '\n' {
            x = x_init;
            y += 8;
        } else {
            draw_char(frame, width, x, y, ch);
            x += 8;
        }
    }
}

#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default, Debug)]
struct App {
    window: Option<Box<dyn Window>>,

    force: f32,

    posx: f32,
    posy: f32,

    twist: f32,

    spherical_tilt_x: f32,
    spherical_tilt_y: f32,

    distance: f32,
}

#[path = "util/fill.rs"]
mod fill;

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(not(web_platform))]
        let window_attributes = WindowAttributes::default();
        #[cfg(web_platform)]
        let window_attributes = WindowAttributes::default().with_append(true);
        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        }
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let window = match self.window.as_ref() {
            Some(window) => window,
            None => return,
        };

        // Suppress warning for unused properties in struct-like enum bindings.
        match event {
            WindowEvent::CloseRequested => {
                println!("Close was requested; stopping");
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(_) => {
                self.window.as_ref().expect("resize event without a window").request_redraw();
            },
            WindowEvent::PointerMoved {
                position,
                source:
                    winit::event::PointerSource::Pen {
                        force,
                        tilt_x,
                        tilt_y,
                        tilt_azimuth,
                        tilt_altitude,
                        state_info: winit::event::PenStateInfo { twist, distance, .. },
                        ..
                    },
                ..
            } => {
                self.posx = position.x as f32;
                self.posy = position.y as f32;
                self.force = force.map(|x| x.normalized()).unwrap_or(0.0) as f32;
                if let (Some(x), Some(y)) = (tilt_x, tilt_y) {
                    let y_rad = (x as f32).to_radians();
                    let x_rad = (y as f32).to_radians();

                    // Clamp to ~89.95 degrees because 90 degree tilts point at points that
                    // can't be described in terms of order-independent spherical tilt.
                    let xoff = -(y_rad.clamp(-1.57, 1.57)).tan();
                    let yoff = -(x_rad.clamp(-1.57, 1.57)).tan();

                    // Normalize from point on a plane to a point on a sphere.
                    let d = (xoff * xoff + yoff * yoff + 1.0).sqrt();
                    self.spherical_tilt_x = xoff / d;
                    self.spherical_tilt_y = yoff / d;
                }
                // Azimuth/altitude are more precise, so override the previous result with them if
                // they exist.
                if let (Some(a), Some(o)) = (tilt_azimuth, tilt_altitude) {
                    let a_rad = (a as f32).to_radians();
                    let o_rad = (o as f32).to_radians();

                    self.spherical_tilt_x = o_rad.sin() * a_rad.sin();
                    self.spherical_tilt_y = o_rad.sin() * -a_rad.cos();
                }
                if let Some(twist) = twist {
                    self.twist = twist;
                } else {
                    self.twist = (self.spherical_tilt_x.atan2(-self.spherical_tilt_y)).to_degrees();
                }

                self.distance = match distance {
                    Some(winit::event::Distance::Unnormalized(x)) => x as f32,
                    Some(winit::event::Distance::Normalized(x)) => x as f32,
                    _ => 0.0,
                };
            },
            WindowEvent::RedrawRequested => {
                window.pre_present_notify();
                fill::fill_window_with_fn(
                    &**self.window.as_ref().unwrap(),
                    |frame, stride, scale| {
                        let frame_w = stride;
                        let frame_h = frame.len() / stride;

                        frame.fill(0xff181818);

                        draw_text(
                            frame,
                            stride,
                            20,
                            50,
                            &format!(
                                "x: {}\ny: {}\ntilt x: {:.3}\ntilt y: {:.3}\ntwist: {}\npressure: \
                                 {:.3}\nheight: {:.3}",
                                self.posx.round(),
                                self.posy.round(),
                                self.spherical_tilt_x,
                                self.spherical_tilt_y,
                                self.twist.round(),
                                self.force,
                                self.distance,
                            ),
                        );

                        let xoff = self.spherical_tilt_x * 400.0;
                        let yoff = self.spherical_tilt_y * 400.0;

                        let mut draw_line = |xpos: f32, ypos: f32, xoff: f32, yoff: f32| {
                            for i in 0..160 {
                                let i = i as f32 * (1.0 / 160.0);

                                let x = (xpos as f32 + xoff * i) / scale as f32;
                                let y = (ypos as f32 + yoff * i) / scale as f32;

                                let xpart = x.clamp(0.0, frame_w as f32 - 1.0) as usize;
                                let ypart = y.clamp(0.0, frame_h as f32 - 1.0) as usize * stride;
                                frame[ypart + xpart] = 0xffffffff;
                            }
                        };

                        // "This is where the cursor is" indicator.
                        draw_line(self.posx, self.posy, 10.0, 10.0);
                        draw_line(self.posx, self.posy, 0.0, 15.0);
                        draw_line(self.posx, self.posy + 15.0, 10.0, -5.0);
                        draw_line(self.posx + 5.0, self.posy + 12.5, 4.0, 8.0);

                        // Tilt indicator.
                        draw_line(self.posx, self.posy, xoff, yoff);

                        let dx = (self.twist * (1.57079632679 / 90.0)).sin();
                        let dy = -(self.twist * (1.57079632679 / 90.0)).cos();

                        // Double-checking line for whether the azimuth is being respected.
                        // Only works on devices with no twist support, and only certain ones.
                        // draw_line(self.posx, self.posy + 10.0, dx * 400.0, dy * 400.0);

                        // Twist indicator.
                        draw_line(self.posx + xoff, self.posy + yoff, dx * 40.0, dy * 40.0);
                        draw_line(self.posx + xoff, self.posy + yoff, dy * 40.0, -dx * 40.0);
                        draw_line(self.posx + xoff, self.posy + yoff, -dy * 40.0, dx * 40.0);

                        // Pressure indicator.
                        let d = 100.0 * self.force;
                        draw_line(self.posx + xoff - d, self.posy + yoff + d, d * 2.0, 0.0);
                        draw_line(self.posx + xoff - d, self.posy + yoff - d, d * 2.0, 0.0);
                    },
                );
                window.request_redraw();
                #[cfg(not(web_platform))]
                {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            },
            _ => (),
        }
    }
}

pub fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new()?;

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App::default())?;

    Ok(())
}

#[cfg(web_platform)]
use wasm_bindgen::prelude::wasm_bindgen;
#[cfg(web_platform)]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new().unwrap();

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App::default()).unwrap();

    Ok(())
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    tracing::init();

    let event_loop = EventLoop::builder().with_android_app(app).build().unwrap();
    event_loop.run_app(App::default()).unwrap();
}
