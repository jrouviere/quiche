use std::time::Duration;
use std::time::Instant;

use crate::packet;
use crate::recovery::Recovery;
use crate::recovery::RecoveryOps;
use crate::StartupExit;

use super::app::AppSimulator;
use super::network::NetworkSimulator;

pub fn run(
    simu_duration: usize, mut recovery: Recovery, mut app: AppSimulator,
    mut network: NetworkSimulator,
) -> Stats {
    let start = Instant::now();

    recovery.update_app_limited(false);

    // this is the simulation "now", not real time
    let mut now = start;

    let mut tick_stats = Vec::new();

    // Simulation loop:
    // Pick the event happening first, from:
    // - CC wants to send next packet
    // - network wants to send an ack
    // - app wants to send some data (not implemented yet)
    // execute the event and keep going

    for _ in 0..simu_duration {
        // load next release time
        let next_send = recovery.get_next_release_time();

        // load next ack time
        let next_ack = network.get_next_simu_ack_time();

        // next loss detection call
        let next_loss = recovery.loss_detection_timer();

        let send_time = if recovery.cwnd_available() < 1200 {
            None
        } else {
            match next_send.time(now) {
                Some(t) => Some(t),
                None => Some(now),
            }
        };

        let next_action = match find_earliest(&[send_time, next_ack, next_loss]) {
            0 => Action::Send,
            1 => Action::Ack,
            2 => Action::LossDetection,
            _ => unreachable!(),
        };

        match next_action {
            Action::Ack => {
                // move to ack time
                now = next_ack.unwrap();

                let (ranges, ack_delay) = network.get_next_ack(now);
                recovery.on_ack_received(
                    &ranges,
                    ack_delay,
                    packet::Epoch::Application,
                    app.get_handshake(),
                    now,
                    "",
                );
            },

            Action::Send => {
                // move to send time
                now = send_time.unwrap();

                let sent = app.get_next_packet(now);
                recovery.on_packet_sent(
                    sent.clone(),
                    packet::Epoch::Application,
                    app.get_handshake(),
                    now,
                    "",
                );
                network.enqueue_packet(sent);
            },

            Action::LossDetection => {
                // move to loss time
                now = next_loss.unwrap();

                recovery.on_loss_detection_timeout(app.get_handshake(), now, "");
            },
        }

        tick_stats.push(TickStats {
            now,
            elapsed: now.duration_since(start).as_millis() as f64,
            cwnd: recovery.cwnd(),
            in_flight_count: recovery.in_flight_count(packet::Epoch::Application),
            in_flight_bytes: recovery.bytes_in_flight(),
            delivery_rate: recovery.delivery_rate().to_bytes_per_second(),
            rtt: recovery.rtt(),
            min_rtt: recovery.min_rtt().unwrap_or(recovery.rtt()),
            max_rtt: recovery.max_rtt().unwrap_or(recovery.rtt()),
        });
    }

    Stats {
        startup_exit: recovery.startup_exit(),
        ticks: tick_stats,
    }
}

#[derive(Debug)]
pub struct TickStats {
    pub now: Instant,
    pub elapsed: f64,
    pub cwnd: usize,
    pub in_flight_count: usize,
    pub in_flight_bytes: usize,
    pub delivery_rate: u64,
    pub rtt: Duration,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
}

#[derive(Debug)]
pub struct Stats {
    pub startup_exit: Option<StartupExit>,
    pub ticks: Vec<TickStats>,
}

#[derive(Debug)]
enum Action {
    Ack,
    Send,
    LossDetection,
}

fn find_earliest(instants: &[Option<Instant>]) -> usize {
    let mut earliest = None;

    for (idx, inst) in instants.iter().enumerate() {
        match (inst, earliest) {
            (Some(_), None) => earliest = Some(idx),
            (Some(inst), Some(early)) =>
                if *inst < instants[early].unwrap() {
                    earliest = Some(idx);
                },
            _ => continue,
        }
    }

    return earliest.unwrap();
}
