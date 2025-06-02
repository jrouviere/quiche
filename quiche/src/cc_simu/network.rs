use rand::rngs::SmallRng;
use rand::Rng;
use rand::SeedableRng;

use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use crate::ranges::RangeSet;
use crate::ranges::{
    self,
};
use crate::recovery::Sent;

pub struct NetworkSimulator {
    rng: SmallRng,
    ratelimiter: Ratelimiter,
    packets: VecDeque<ToSend>,

    // settings
    latency: Duration,
    loss_ratio: f64,
}

impl NetworkSimulator {
    pub fn new(latency: Duration, baudrate: u64, loss_ratio: f64) -> Self {
        assert!(loss_ratio >= 0.0 && loss_ratio <= 1.0);

        Self {
            rng: SmallRng::seed_from_u64(42),
            latency,
            ratelimiter: Ratelimiter::new(Instant::now(), baudrate),
            loss_ratio,
            packets: VecDeque::new(),
        }
    }

    pub fn enqueue_packet(&mut self, sent: Sent) {
        // simulate packet loss
        let rand = self.rng.gen_range(0.0..1.0);
        if rand < self.loss_ratio {
            return;
        }

        // TODO: We probably should implement something
        // a bit smarter here, and simulate various types of network
        // drop packet if above rate limit
        if self
            .ratelimiter
            .try_take(sent.time_sent, 8 * sent.size as u64)
            .is_none()
        {
            return;
        }

        // simulate network delays
        let ack_at = sent.time_sent + self.latency;
        self.packets.push_back(ToSend { ack_at, sent });

        // TODO: check for reordering
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

/// Super basic token bucket implementation
/// without using system timer
struct Ratelimiter {
    rate: u64,
    tokens: u64,
    next_refill: Instant,
}

impl Ratelimiter {
    fn new(now: Instant, mut rate: u64) -> Self {
        rate = rate / 1000; // bucket granularity in milliseconds
        Self {
            rate,
            tokens: rate,
            next_refill: now,
        }
    }

    fn try_take(&mut self, now: Instant, count: u64) -> Option<()> {
        if count > self.rate {
            return None;
        }

        if now > self.next_refill {
            self.next_refill = now + Duration::from_millis(1);
            self.tokens = self.rate - count;
            Some(())
        } else {
            if self.tokens >= count {
                self.tokens -= count;
                Some(())
            } else {
                None
            }
        }
    }
}
