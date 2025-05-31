use std::time::Duration;
use std::time::Instant;

use eframe::egui::Ui;
use eframe::egui::{self, vec2, Align, Layout, Margin};
use egui_plot::{Line, Plot, PlotPoints};

use crate::cc_simu::app::AppSimulator;
use crate::cc_simu::network::NetworkSimulator;
use crate::cc_simu::simu;
use crate::recovery::{Recovery, RecoveryConfig};
use crate::CongestionControlAlgorithm;

use super::simu::Stats;

/// Start GUI app
pub fn gui_main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CC simulation",
        options,
        Box::new(|_cc| Ok(Box::<CCSimuGui>::default())),
    )
}

struct CCSimuGui {
    init_cwnd: usize,
    hystart: bool,
    pacing: bool,
    cc_algo: CongestionControlAlgorithm,
    latency: u64,
    bitrate: f64,
    loss_percent: f64,
    jitter: u32,
    simu_length: usize,
}

impl Default for CCSimuGui {
    fn default() -> Self {
        Self {
            init_cwnd: 10,
            hystart: true,
            pacing: true,
            cc_algo: CongestionControlAlgorithm::CUBIC,
            latency: 10,
            bitrate: 1000.0,
            loss_percent: 0.0,
            jitter: 0,
            simu_length: 2500,
        }
    }
}

impl eframe::App for CCSimuGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            self.display_settings(ui);

            // run
            let before_run = Instant::now();
            let stats = self.run();
            ui.label(format!(
                "simulated in {:.1}ms",
                1000.0 * before_run.elapsed().as_secs_f64()
            ));

            // plot stats
            self.display_graph(ui, &stats);
        });
    }
}

impl CCSimuGui {
    fn display_settings(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            egui::Frame::new()
                .inner_margin(Margin::symmetric(10, 10))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("Simulation");
                        ui.add(
                            egui::Slider::new(
                                &mut self.simu_length,
                                1000..=10000,
                            )
                            .text("length (events)"),
                        );
                    });

                    ui.separator();
                });

            egui::Frame::new()
                .inner_margin(Margin::symmetric(10, 10))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("CC config");
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_label("")
                                .selected_text(cc_algo_to_str(self.cc_algo))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.cc_algo,
                                        CongestionControlAlgorithm::Reno,
                                        cc_algo_to_str(
                                            CongestionControlAlgorithm::Reno,
                                        ),
                                    );
                                    ui.selectable_value(
                                        &mut self.cc_algo,
                                        CongestionControlAlgorithm::CUBIC,
                                        cc_algo_to_str(
                                            CongestionControlAlgorithm::CUBIC,
                                        ),
                                    );
                                    ui.selectable_value(
                                        &mut self.cc_algo,
                                        CongestionControlAlgorithm::BBR,
                                        cc_algo_to_str(
                                            CongestionControlAlgorithm::BBR,
                                        ),
                                    );
                                    ui.selectable_value(
                                        &mut self.cc_algo,
                                        CongestionControlAlgorithm::BBR2,
                                        cc_algo_to_str(
                                            CongestionControlAlgorithm::BBR2,
                                        ),
                                    );
                                    ui.selectable_value(
                                        &mut self.cc_algo,
                                        CongestionControlAlgorithm::Bbr2Gcongestion,
                                        cc_algo_to_str(
                                            CongestionControlAlgorithm::Bbr2Gcongestion,
                                        ),
                                    );
                                });
                        });
                        ui.add(
                            egui::Slider::new(&mut self.init_cwnd, 10..=250)
                                .text("icwnd"),
                        );
                        ui.add(
                            egui::Checkbox::new(&mut self.hystart, "HyStart"),
                        );
                        ui.add(
                            egui::Checkbox::new(&mut self.pacing, "Pacing"),
                        );
                    });
                    ui.separator();
                });

            egui::Frame::new()
                .inner_margin(Margin::symmetric(10, 10))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.heading("Network simulator");
                        ui.add(
                            egui::Slider::new(&mut self.latency, 1..=500)
                                .text("latency (ms)"),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.bitrate, 1.0..=5000.0)
                                .fixed_decimals(0)
                                .step_by(1.0)
                                .text("rate limit (mbps)"),
                        );
                        ui.add(
                            egui::Slider::new(
                                &mut self.loss_percent,
                                0.0..=100.0,
                            )
                            .fixed_decimals(3)
                            .step_by(0.01)
                            .text("random loss %"),
                        );
                        // ui.add(egui::Slider::new(&mut self.jitter, 0..=200).text("jitter"));
                    });
                    ui.separator();
                });
        });
    }

    fn display_graph(&self, ui: &mut Ui, stats: &Stats) {
        let cwnd: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.cwnd as f64])
            .collect();

        let line_cwnd = Line::new("cwnd", cwnd);

        let rtt: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.rtt.as_millis() as f64])
            .collect();
        let line_rtt = Line::new("rtt", rtt);
        let min_rtt: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.min_rtt.as_millis() as f64])
            .collect();
        let line_min_rtt = Line::new("min_rtt", min_rtt);
        let max_rtt: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.max_rtt.as_millis() as f64])
            .collect();
        let line_max_rtt = Line::new("max_rtt", max_rtt);

        let delivery_rate: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.delivery_rate as f64])
            .collect();
        let line_delivery_rate = Line::new("delivery_rate", delivery_rate);

        let in_flight: PlotPoints = stats
            .ticks
            .iter()
            .map(|v| [v.elapsed, v.in_flight_count as f64])
            .collect();
        let line_in_flight = Line::new("in_flight", in_flight);

        egui::Grid::new("plot grid").show(ui, |ui| {
            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("cwnd");

                        Plot::new("stats_cwnd").show(ui, |plot_ui| {
                            plot_ui.line(line_cwnd);
                        });
                    })
                });

            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("rtt");

                        Plot::new("stats_rtt").show(ui, |plot_ui| {
                            plot_ui.line(line_rtt);
                            plot_ui.line(line_min_rtt);
                            plot_ui.line(line_max_rtt);
                        });
                    })
                });

            ui.end_row();

            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("delivery_rate");

                        Plot::new("stats_delivery_rate").show(ui, |plot_ui| {
                            plot_ui.line(line_delivery_rate);
                        });
                    })
                });

            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("in_flight");

                        Plot::new("stats_in_flight").show(ui, |plot_ui| {
                            plot_ui.line(line_in_flight);
                        });
                    })
                });

            ui.end_row();

            ui.add(egui::Label::new(format!(
                "startup exit: {:?}",
                stats.startup_exit
            )))
        });
    }

    fn run(&self) -> Stats {
        let recovery = Recovery::new_with_config(&RecoveryConfig {
            max_send_udp_payload_size: 1200,
            max_ack_delay: Duration::ZERO,
            cc_algorithm: self.cc_algo,
            custom_bbr_params: None,
            hystart: self.hystart,
            pacing: self.pacing,
            max_pacing_rate: None,
            initial_congestion_window_packets: self.init_cwnd,
        });

        let app = AppSimulator::new();
        let network = NetworkSimulator::new(
            Duration::from_millis(self.latency),
            (1024.0 * 1024.0 * self.bitrate) as u64,
            self.loss_percent / 100.0,
        );

        simu::run(self.simu_length, recovery, app, network)
    }
}

fn cc_algo_to_str(algo: CongestionControlAlgorithm) -> &'static str {
    match algo {
        CongestionControlAlgorithm::Reno => "reno",
        CongestionControlAlgorithm::CUBIC => "cubic",
        CongestionControlAlgorithm::BBR => "bbr",
        CongestionControlAlgorithm::BBR2 => "bbr2",
        CongestionControlAlgorithm::Bbr2Gcongestion => "bbr2_gcongestion",
    }
}
