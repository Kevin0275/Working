use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

// Needed for plotting
use egui_plot::{Line, Plot, PlotPoints, PlotBounds};

#[derive(Default)]
struct AudioData {
    samples: VecDeque<f32>,
    rms: f32,
    amplitude: f32,
}

fn main() -> Result<(), eframe::Error> {
    let data = Arc::new(Mutex::new(AudioData::default()));
    start_audio_thread(Arc::clone(&data));

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "🎧 Mic Visualizer",
        native_options,
        Box::new(|_cc| Box::new(AppState { data })),
    )
}

struct AppState {
    data: Arc<Mutex<AudioData>>,
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎙 Live Microphone Input");

            let data = self.data.lock().unwrap();
            ui.label(format!(
                "RMS: {:.4} | Amplitude: {:.4}",
                data.rms, data.amplitude
            ));

            let plot = Plot::new("audio_plot")
                .view_aspect(2.0)
                .allow_scroll(false)
                .allow_zoom(false);

            plot.show(ui, |plot_ui| {
                // Set fixed plot bounds
                plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                    [0.0, -0.1],   // X min, Y min
                    [500.0, 0.1],  // X max, Y max
                ));

                let points: PlotPoints = data
                    .samples
                    .iter()
                    .enumerate()
                    .map(|(i, &s)| [i as f64, s as f64])
                    .collect();

                plot_ui.line(Line::new(points));
            });
        });

        ctx.request_repaint_after(Duration::from_millis(30));
    }
}

fn start_audio_thread(shared: Arc<Mutex<AudioData>>) {
    thread::spawn(move || {
        let host = cpal::default_host();
        let device = host.default_input_device().expect("No input device found");
        let config = device.default_input_config().unwrap();
        let channels = config.channels() as usize;

        let sample_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut buffer = shared.lock().unwrap();

            let mut sum = 0.0;
            let mut max: f32 = 0.0;

            for frame in data.chunks(channels) {
                let s = frame[0];
                sum += s * s;
                max = max.max(s.abs());
                buffer.samples.push_back(s);

                if buffer.samples.len() > 500 {
                    buffer.samples.pop_front();
                }
            }

            buffer.rms = (sum / data.len() as f32).sqrt();
            buffer.amplitude = max;
        };

        let err_fn = |err| eprintln!("Stream error: {}", err);
        let stream = device
            .build_input_stream(&config.into(), sample_fn, err_fn, None)
            .unwrap();

        stream.play().unwrap();

        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}
