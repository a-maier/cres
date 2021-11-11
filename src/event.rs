use crate::four_vector::FourVector;

use std::default::Default;
use std::convert::From;

use noisy_float::prelude::*;

pub type MomentumSet = Vec<FourVector>;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
pub struct EventBuilder {
    id: usize,
    weight: N64,

    outgoing_by_pid: Vec<(i32, FourVector)>,
}

impl EventBuilder {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            weight: n64(0.),
            outgoing_by_pid: Vec::new()
        }
    }

    pub fn with_capacity(id: usize, cap: usize) -> Self {
        Self {
            id,
            weight: n64(0.),
            outgoing_by_pid: Vec::with_capacity(cap)
        }
    }

    pub fn add_outgoing(&mut self, pid: i32, p: FourVector) -> &mut Self {
        self.outgoing_by_pid.push((pid, p));
        self
    }

    pub fn weight(&mut self, weight: N64) -> &mut Self {
        self.weight = weight;
        self
    }

    pub fn build(self) -> Event {
        let outgoing_by_pid = compress_outgoing(self.outgoing_by_pid);
        Event {
            id: self.id,
            weight: self.weight,
            outgoing_by_pid
        }
    }
}

impl From<EventBuilder> for Event {
    fn from(b: EventBuilder) -> Self {
        b.build()
    }
}

fn compress_outgoing(mut out: Vec<(i32, FourVector)>) -> Vec<(i32, Vec<FourVector>)> {
    out.sort_unstable_by(|a, b| b.cmp(a));
    let mut outgoing_by_pid : Vec<(i32, Vec<_>)> = Vec::new();
    for (id, p) in out {
        match outgoing_by_pid.last_mut() {
            Some((pid, v)) if *pid == id => v.push(p),
            _ => outgoing_by_pid.push((id, vec![p]))
        }
    }
    outgoing_by_pid
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Default)]
pub struct Event {
    id: usize,
    pub weight: N64,

    outgoing_by_pid: Vec<(i32, MomentumSet)>,
}

const EMPTY_SLICE: &'static [FourVector] = &[];

impl Event {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn outgoing(&self) -> &[(i32, MomentumSet)] {
        self.outgoing_by_pid.as_slice()
    }

    pub fn outgoing_with_pid(&self, pid: i32) -> &[FourVector] {
        let idx = self.outgoing_by_pid.binary_search_by(|probe| pid.cmp(&probe.0));
        if let Ok(idx) = idx {
            &self.outgoing_by_pid[idx].1
        } else {
            EMPTY_SLICE
        }
    }

    pub fn into_outgoing(self) -> Vec<(i32, MomentumSet)> {
        self.outgoing_by_pid
    }
}
