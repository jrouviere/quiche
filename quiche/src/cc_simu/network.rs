use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use crate::ranges::RangeSet;
use crate::ranges::{self};
use crate::recovery::Sent;

pub struct NetworkSimulator {
    rng: SmallRng,
    packets: VecDeque<ToSend>,

    // settings
    latency: Duration,
    bitrate: f64, // bits per second
    capacity_packets: usize,
    loss_ratio: f64,
}

impl NetworkSimulator {
    pub fn new(
        capacity_packets: usize, latency: Duration, bitrate: f64, loss_ratio: f64,
    ) -> Self {
        assert!(loss_ratio >= 0.0 && loss_ratio <= 1.0);

        Self {
            rng: SmallRng::seed_from_u64(42),
            latency,
            bitrate,
            capacity_packets,
            loss_ratio,
            packets: VecDeque::new(),
        }
    }

    pub fn enqueue_packet(&mut self, sent: Sent) {
        // if we are above the network capacity, drop the packet
        if self.packets.len() >= self.capacity_packets {
            return;
        }

        // simulate packet loss
        if self.rng.gen_range(0.0..1.0) < self.loss_ratio {
            return;
        }

        // simulate network delays
        let time_to_send_packet =
            Duration::from_secs_f64(8.0 * sent.size as f64 / self.bitrate);

        let ack_at = if let Some(last) = self.packets.back() {
            // queue directly after the last packet if there is one
            std::cmp::max(
                sent.time_sent + self.latency + time_to_send_packet,
                last.ack_at + time_to_send_packet,
            )
        } else {
            // first packet in the queue, consider latency
            sent.time_sent + self.latency + time_to_send_packet
        };

        self.packets.push_back(ToSend { ack_at, sent });
    }

    pub fn get_next_simu_ack_time(&self) -> Option<Instant> {
        self.packets.front().map(|ts| ts.ack_at)
    }

    pub fn get_next_ack(&mut self, now: Instant) -> (RangeSet, u64) {
        let mut acked = ranges::RangeSet::default();

        while let Some(to_send) = self.packets.front() {
            if to_send.ack_at <= now {
                let to_send = self.packets.pop_front().unwrap();
                acked.push_item(to_send.sent.pkt_num);
            } else {
                break;
            }
        }

        (acked, 0)
    }
}

struct ToSend {
    ack_at: Instant,
    sent: Sent,
}
