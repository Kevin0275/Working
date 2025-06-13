use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kiss3d::camera::{FirstPerson};
use kiss3d::event::{Action, Key, WindowEvent};
use kiss3d::light::Light;
use kiss3d::nalgebra::{Point2, Point3, Translation3, Vector3};
use kiss3d::resource::Mesh;
use kiss3d::scene::SceneNode;
use kiss3d::window::Window;

struct SamplePoint {
    position: Point2<f32>,
    amplitude: f32,
}

fn main() {
    let (tx, rx) = mpsc::channel::<f32>();

    // Spawn audio capture thread
    thread::spawn(move || {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device available");
        let config = device.default_input_config().unwrap();
        let channels = config.channels() as usize;

        let stream = device.build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let max = data.chunks(channels)
                    .map(|frame| frame[0].abs())
                    .fold(0.0, f32::max);
                let _ = tx.send(max);
            },
            move |err| eprintln!("Stream error: {}", err),
            None,
        ).unwrap();

        stream.play().unwrap();
        loop {
            thread::sleep(Duration::from_millis(10));
        }
    });

    // Set up 3D window and camera
    let eye = Point3::new(0.0, -2.0, 1.0);
    let at = Point3::origin();
    let mut camera = FirstPerson::new(eye, at);
    let mut window = Window::new("Mic 3D Visualizer");
    window.set_light(Light::StickToCamera);
    window.set_background_color(1.0, 1.0, 1.0);

    // Mic dot
    let mut mic_position = Point2::new(0.0, 0.0);
    let mut mic_node = window.add_sphere(0.015);
    mic_node.set_color(0.0, 1.0, 0.0);

    // Storage
    let mut samples: Vec<SamplePoint> = Vec::new();
    let mut surface_node: Option<SceneNode> = None;
    let mut camera_shift = Vector3::new(0.0, 0.0, 0.0);

    while window.render_with_camera(&mut camera) {
        for event in window.events().iter() {
            if let WindowEvent::Key(key, Action::Press, _) = event.value {
                match key {
                    Key::W => mic_position.y += 0.05,
                    Key::S => mic_position.y -= 0.05,
                    Key::A => mic_position.x -= 0.05,
                    Key::D => mic_position.x += 0.05,
                    Key::Up => camera_shift.y += 0.05,
                    Key::Down => camera_shift.y -= 0.05,
                    Key::Left => camera_shift.x -= 0.05,
                    Key::Right => camera_shift.x += 0.05,
                    Key::Space => {
                        if let Ok(amp) = rx.try_recv() {
                            samples.push(SamplePoint {
                                position: mic_position,
                                amplitude: amp,
                            });
                        }
                    }
                    Key::R => {
                        samples.clear();
                        if let Some(mut node) = surface_node.take() {
                            window.remove_node(&mut node);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Update mic dot and camera
        mic_node.set_local_translation(Translation3::new(mic_position.x, mic_position.y, 0.0));
        camera.translate(&Translation3::from(camera_shift));
        camera_shift = Vector3::new(0.0, 0.0, 0.0);

        // Grid lines (0.1 unit spacing)
        for i in -10..=10 {
            let i = i as f32 * 0.1;
            window.draw_line(&Point3::new(i, -1.0, 0.0), &Point3::new(i, 1.0, 0.0), &Point3::new(1.0, 0.0, 0.0)); // X
            window.draw_line(&Point3::new(-1.0, i, 0.0), &Point3::new(1.0, i, 0.0), &Point3::new(0.0, 0.8, 0.0)); // Y
        }

        // Axes lines
        window.draw_line(&Point3::origin(), &Point3::new(0.3, 0.0, 0.0), &Point3::new(1.0, 0.0, 0.0)); // X
        window.draw_line(&Point3::origin(), &Point3::new(0.0, 0.3, 0.0), &Point3::new(0.0, 1.0, 0.0)); // Y
        window.draw_line(&Point3::origin(), &Point3::new(0.0, 0.0, 1.0), &Point3::new(0.0, 0.0, 1.0)); // Z

        // Convert samples to points
        let points: Vec<Point3<f32>> = samples
            .iter()
            .map(|s| Point3::new(s.position.x, s.position.y, s.amplitude))
            .collect();

        // Draw black points
        for p in &points {
            window.draw_point(p, &Point3::new(0.0, 0.0, 0.0));
        }

        // Connect points with gray lines
        for w in points.windows(2) {
            if let [a, b] = w {
                window.draw_line(a, b, &Point3::new(0.2, 0.2, 0.2));
            }
        }

        // Surface mesh
        if points.len() >= 3 {
            if let Some(mut node) = surface_node.take() {
                window.remove_node(&mut node);
            }

            let vertices = points.clone();
            let indices = (0..vertices.len() - 2)
                .map(|i| Point3::new(i as u16, (i + 1) as u16, (i + 2) as u16))
                .collect();

            let mesh = Mesh::new(vertices, indices, None, None, false);
            let mut node = window.add_mesh(Rc::new(RefCell::new(mesh)), Vector3::new(1.0, 1.0, 1.0));
            node.set_color(0.7, 0.7, 0.7);
            surface_node = Some(node);
        }
    }
}
