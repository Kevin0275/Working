use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel;
use eframe::egui::{self, Slider};
use egui_plot::{Line, Plot, PlotPoints};

fn main() {
    let (sender, receiver) = channel::bounded::<(f32, f32)>(1024);
    let x_position = Arc::new(Mutex::new(0.0));
    let x_clone = Arc::clone(&x_position);

    thread::spawn(move || {
        if let Err(e) = capture_audio(sender, x_clone) {
            eprintln!("Audio thread error: {:?}", e);
        }
    });

    let app = AudioPlotApp {
        receiver,
        values: Vec::new(),
        x_position,
        mic_locked: true, // Default locked
    };

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Amplitude vs X Position",
        native_options,
        Box::new(|_cc| Box::new(app)),
    )
    .expect("Failed to launch GUI");
}

fn capture_audio(
    sender: channel::Sender<(f32, f32)>,
    x_position: Arc<Mutex<f32>>,
) -> Result<()> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("No input device available");
    let config = device.default_input_config()?;

    let stream = device.build_input_stream(
        &config.into(),
        move |data: &[f32], _| {
            if data.is_empty() {
                return;
            }
            let rms = (data.iter().map(|&s| s * s).sum::<f32>() / data.len() as f32).sqrt();
            if rms > 0.01 {
                let x = *x_position.lock().unwrap();
                let _ = sender.send((x, rms));
            }
        },
        move |err| {
            eprintln!("Stream error: {:?}", err);
        },
        None,
    )?;

    stream.play()?;
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

struct AudioPlotApp {
    receiver: channel::Receiver<(f32, f32)>,
    values: Vec<(f32, f32)>,
    x_position: Arc<Mutex<f32>>,
    mic_locked: bool,
}

impl eframe::App for AudioPlotApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Only update sound if unlocked
        if !self.mic_locked {
            while let Ok((x, a)) = self.receiver.try_recv() {
                if a > 0.01 {
                    let x_rounded = (x * 100.0).round() / 100.0;

                    // Always update amplitude at that position
                    if let Some(existing) = self.values.iter_mut().find(|(ex, _)| {
                        (*ex * 100.0).round() / 100.0 == x_rounded
                    }) {
                        existing.1 = a;
                    } else {
                        self.values.push((x_rounded, a));
                    }
                }
            }
        } else {
            // Drain any pending audio data without using it
            while let Ok((_x, _a)) = self.receiver.try_recv() {}
        }

        // Sort X for clean line drawing
        self.values
            .sort_by(|(x1, _), (x2, _)| x1.partial_cmp(x2).unwrap_or(std::cmp::Ordering::Equal));

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Adjust X position manually:");
            let mut x = *self.x_position.lock().unwrap();
            if ui.add(Slider::new(&mut x, 0.0..=100.0).text("X Position")).changed() {
                *self.x_position.lock().unwrap() = x;
            }

            ui.separator();

            // Lock toggle
            ui.horizontal(|ui| {
                ui.label("Lock Mic Position:");
                ui.toggle_value(&mut self.mic_locked, "🔒");
                ui.label(if self.mic_locked {
                    "Locked - No data will be recorded"
                } else {
                    "Unlocked - Recording enabled"
                });
            });

            let plot_points: PlotPoints = self
                .values
                .iter()
                .map(|(x, y)| [*x as f64, *y as f64])
                .collect();

            Plot::new("amplitude_vs_x")
                .view_aspect(2.0)
                .include_y(0.0)
                .include_y(0.2)
                .show(ui, |plot_ui| {
                    plot_ui.line(Line::new(plot_points).name("RMS Amplitude"));
                });
        });

        ctx.request_repaint();
    }
}
