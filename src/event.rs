use noisy_float::prelude::*;
use std::default::Default;

use crate::four_vector::FourVector;

pub type MomentumSet = Vec<FourVector>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Default)]
pub struct Event {
    pub id: usize,
    pub weight: N64,

    pub(crate) outgoing_by_pid: Vec<(i32, MomentumSet)>,
}

impl Event {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_outgoing(&mut self, id: i32, p: FourVector) {
        let id_pos = self
            .outgoing_by_pid
            .binary_search_by(|(type_id, _)| type_id.cmp(&id));
        let id_pos = match id_pos {
            Ok(pos) => pos,
            Err(pos) => {
                self.outgoing_by_pid.insert(pos, (id, Vec::new()));
                pos
            }
        };

        let (_, type_array) = &mut self.outgoing_by_pid[id_pos];
        let pos = type_array
            .binary_search_by(|a| p.cmp(a))
            .unwrap_or_else(|x| x);
        type_array.insert(pos, p);
        // debug_assert!(type_array.is_sorted_by(|a, b| b.cmp(a)));
    }
}
