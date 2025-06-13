use std::time::Duration;
use std::time::Instant;

use eframe::egui::vec2;
use eframe::egui::Align;
use eframe::egui::Layout;
use eframe::egui::Margin;
use eframe::egui::Ui;
use eframe::egui::{self};
use egui_plot::Line;
use egui_plot::Plot;
use egui_plot::PlotPoints;

use crate::cc_simu::app::AppSimulator;
use crate::cc_simu::network::NetworkSimulator;
use crate::cc_simu::simu;
use crate::recovery::Recovery;
use crate::recovery::RecoveryConfig;
use crate::BbrParams;
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
    // simu
    simu_length: usize,

    // algo
    cc_algo: CongestionControlAlgorithm,
    init_cwnd: usize,
    hystart: bool,
    pacing: bool,

    // bbr conf
    startup_cwnd_gain: f32,
    drain_cwnd_gain: f32,

    // network
    capacity: usize,
    latency: u64,
    bitrate_mbps: f64,
    loss_percent: f64,
}

impl Default for CCSimuGui {
    fn default() -> Self {
        Self {
            simu_length: 10000,
            cc_algo: CongestionControlAlgorithm::Bbr2Gcongestion,
            init_cwnd: 10,
            hystart: true,
            pacing: true,
            startup_cwnd_gain: 2.89,
            drain_cwnd_gain: 2.89,
            capacity: 1000,
            latency: 10,
            bitrate_mbps: 100.0,
            loss_percent: 0.0,
        }
    }
}

impl eframe::App for CCSimuGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // run
            let before_run = Instant::now();

            crate::recovery::gcongestion::bbr::bandwidth_sampler::USE_A0_FIX
                .store(true, std::sync::atomic::Ordering::Relaxed);
            let stats1 = self.run();

            crate::recovery::gcongestion::bbr::bandwidth_sampler::USE_A0_FIX
                .store(false, std::sync::atomic::Ordering::Relaxed);
            let stats2 = self.run();

            let total_simulated1 =
                stats1.ticks.last().map(|v| v.elapsed).unwrap_or(0.0);
            let total_simulated2 =
                stats2.ticks.last().map(|v| v.elapsed).unwrap_or(0.0);

            ui.horizontal(|ui| {
                self.display_settings(ui);

                ui.vertical(|ui| {
                    ui.heading("Results");
                    ui.label(format!(
                        "simulated {:.1}ms in {:.1}ms",
                        total_simulated1 + total_simulated2,
                        1000.0 * before_run.elapsed().as_secs_f64()
                    ));
                    ui.label(format!("startup exit1: {:?}", stats1.startup_exit));
                    ui.label(format!("startup exit2: {:?}", stats2.startup_exit));
                });
            });

            // plot stats
            self.display_graph(ui, &stats1, &stats2);
        });
    }
}

impl CCSimuGui {
    fn display_settings(&mut self, ui: &mut Ui) {
        egui::Frame::new()
            .inner_margin(Margin::symmetric(10, 10))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Simulation");
                    ui.add(
                        egui::Slider::new(&mut self.simu_length, 1000..=100000)
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

                         ui.add(
                            egui::Slider::new(&mut self.startup_cwnd_gain, 1.0..=3.0)
                                .text("startup_cwnd_gain"),
                        );
                         ui.add(
                            egui::Slider::new(&mut self.drain_cwnd_gain, 1.0..=3.0)
                                .text("drain_cwnd_gain"),
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
                        egui::Slider::new(&mut self.capacity, 100..=10000)
                            .text("network buffer (packets)"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.bitrate_mbps, 1.0..=2500.0)
                            .fixed_decimals(0)
                            .step_by(1.0)
                            .text("rate limit (mbps)"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.loss_percent, 0.0..=100.0)
                            .fixed_decimals(3)
                            .step_by(0.01)
                            .text("random loss %"),
                    );
                    // ui.add(egui::Slider::new(&mut self.jitter,
                    // 0..=200).text("jitter"));
                });
                ui.separator();
            });
    }

    fn display_graph(&self, ui: &mut Ui, stats1: &Stats, stats2: &Stats) {
        let lines1 = extract_stats(stats1, "_new");
        let lines2 = extract_stats(stats2, "_old");

        egui::Grid::new("plot grid").show(ui, |ui| {
            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("cwnd (packets)");

                        Plot::new("stats_cwnd").show(ui, |plot_ui| {
                            plot_ui.line(lines1.cwnd);
                            plot_ui.line(lines2.cwnd);
                        });
                    })
                });

            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("RTT (ms)");

                        Plot::new("stats_rtt").show(ui, |plot_ui| {
                            plot_ui.line(lines1.rtt);
                            plot_ui.line(lines1.min_rtt);
                            plot_ui.line(lines1.max_rtt);
                            plot_ui.line(lines2.rtt);
                            plot_ui.line(lines2.min_rtt);
                            plot_ui.line(lines2.max_rtt);
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
                        ui.heading("delivery rate (mbps)");

                        Plot::new("stats_delivery_rate").show(ui, |plot_ui| {
                            plot_ui.line(lines1.delivery_rate);
                            plot_ui.line(lines2.delivery_rate);
                        });
                    })
                });

            egui::Frame::new()
                .outer_margin(Margin::ZERO)
                .inner_margin(Margin::symmetric(5, 5))
                .show(ui, |ui| {
                    ui.set_max_size(vec2(600.0, 300.0));
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.heading("in flight (packets)");

                        Plot::new("stats_in_flight").show(ui, |plot_ui| {
                            plot_ui.line(lines1.in_flight);
                            plot_ui.line(lines2.in_flight);
                        });
                    })
                });

            ui.end_row();
        });
    }

    fn run(&self) -> Stats {
        let recovery = Recovery::new_with_config(&RecoveryConfig {
            max_send_udp_payload_size: 1200,
            max_ack_delay: Duration::ZERO,
            cc_algorithm: self.cc_algo,
            custom_bbr_params: Some(BbrParams {
                startup_cwnd_gain: Some(self.startup_cwnd_gain),
                drain_cwnd_gain: Some(self.drain_cwnd_gain),
                ..Default::default()
            }),
            hystart: self.hystart,
            pacing: self.pacing,
            max_pacing_rate: None,
            initial_congestion_window_packets: self.init_cwnd,
        });

        let app = AppSimulator::new();
        let network = NetworkSimulator::new(
            self.capacity,
            Duration::from_millis(self.latency),
            1024.0 * 1024.0 * self.bitrate_mbps,
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

struct Lines<'a> {
    cwnd: Line<'a>,
    rtt: Line<'a>,
    min_rtt: Line<'a>,
    max_rtt: Line<'a>,
    delivery_rate: Line<'a>,
    in_flight: Line<'a>,
}
fn extract_stats<'a>(stats: &'a Stats, suffix: &str) -> Lines<'a> {
    let cwnd: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| [v.elapsed, v.cwnd as f64 / 1200.0])
        .collect();

    let line_cwnd = Line::new(format!("cwnd{suffix}"), cwnd);

    let rtt: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| [v.elapsed, v.rtt.as_millis() as f64])
        .collect();
    let line_rtt = Line::new(format!("rtt{suffix}"), rtt);
    let min_rtt: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| [v.elapsed, v.min_rtt.as_millis() as f64])
        .collect();
    let line_min_rtt = Line::new(format!("min_rtt{suffix}"), min_rtt);
    let max_rtt: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| [v.elapsed, v.max_rtt.as_millis() as f64])
        .collect();
    let line_max_rtt = Line::new(format!("max_rtt{suffix}"), max_rtt);

    let delivery_rate: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| {
            [
                v.elapsed,
                8.0 * v.delivery_rate as f64 / 1024.0 / 1024.0 as f64,
            ]
        })
        .collect();
    let line_delivery_rate =
        Line::new(format!("delivery_rate{suffix}"), delivery_rate);

    let in_flight: PlotPoints = stats
        .ticks
        .iter()
        .map(|v| [v.elapsed, v.in_flight_count as f64])
        .collect();
    let line_in_flight = Line::new(format!("in_flight{suffix}"), in_flight);

    return Lines {
        cwnd: line_cwnd,
        rtt: line_rtt,
        min_rtt: line_min_rtt,
        max_rtt: line_max_rtt,
        delivery_rate: line_delivery_rate,
        in_flight: line_in_flight,
    };
}
