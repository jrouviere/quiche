use smallvec::smallvec;
use std::time::Instant;

use crate::recovery::HandshakeStatus;
use crate::recovery::Sent;

pub struct AppSimulator {
    pkt_num: u64,
}

impl AppSimulator {
    pub fn new() -> Self {
        Self { pkt_num: 0 }
    }

    pub fn get_handshake(&self) -> HandshakeStatus {
        HandshakeStatus {
            has_handshake_keys: true,
            peer_verified_address: true,
            completed: true,
        }
    }

    pub fn get_next_packet(&mut self, now: Instant) -> Sent {
        self.pkt_num += 1;
        Sent {
            pkt_num: self.pkt_num,
            frames: smallvec![],
            time_sent: now,
            time_acked: None,
            time_lost: None,
            size: 1200,
            ack_eliciting: true,
            in_flight: true,
            delivered: 0,
            delivered_time: now,
            first_sent_time: now,
            is_app_limited: false,
            tx_in_flight: 0,
            lost: 0,
            has_data: false,
            pmtud: false,
        }
    }
}
