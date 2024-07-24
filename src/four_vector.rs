use jetty::PseudoJet;
use noisy_float::prelude::*;
use serde::{Deserialize, Serialize};

/// A basic four-vector
///
/// The zero component is the energy/time component. The remainder are
/// the spatial components
#[derive(
    Deserialize,
    Serialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Clone,
    Copy,
    Default,
)]
pub struct FourVector {
    pt: N64,
    p: [N64; 4],
}

impl FourVector {
    /// Construct a new four-vector
    pub fn new() -> Self {
        Self::default()
    }

    /// The euclidean norm \sqrt{\sum v_\mu^2} with \mu = 0,1,2,3
    pub fn euclid_norm(&self) -> N64 {
        self.euclid_norm_sq().sqrt()
    }
    /// The square \sum v_\mu^2 with \mu = 0,1,2,3 of the euclidean norm
    pub fn euclid_norm_sq(&self) -> N64 {
        self.p.iter().map(|e| *e * *e).sum()
    }

    /// The spatial norm \sqrt{\sum v_i^2} with i = 1,2,3
    pub fn spatial_norm(&self) -> N64 {
        self.spatial_norm_sq().sqrt()
    }

    /// The square \sum v_i^2 with i = 1,2,3 of the spatial norm
    pub fn spatial_norm_sq(&self) -> N64 {
        self.p.iter().skip(1).map(|e| *e * *e).sum()
    }

    /// The scalar transverse momentum
    pub fn pt(&self) -> N64 {
        self.pt
    }

    const fn len() -> usize {
        4
    }

    fn update_pt(&mut self) {
        self.pt = (self.p[1] * self.p[1] + self.p[2] * self.p[2]).sqrt();
    }

    /// The invariant mass \sqrt{v_0^2 - \sum v_i^2} with i = 1,2,3
    pub fn m(&self) -> N64 {
        self.m_sq().sqrt()
    }

    /// The invariant mass square v_0^2 - \sum v_i^2 with i = 1,2,3
    pub fn m_sq(&self) -> N64 {
        self.p[0] * self.p[0] - self.spatial_norm()
    }
}

impl std::convert::From<[N64; 4]> for FourVector {
    fn from(p: [N64; 4]) -> FourVector {
        let mut res = FourVector {
            p,
            pt: std::default::Default::default(),
        };
        res.update_pt();
        res
    }
}

impl std::ops::Index<usize> for FourVector {
    type Output = N64;

    fn index(&self, i: usize) -> &Self::Output {
        &self.p[i]
    }
}

impl std::ops::AddAssign for FourVector {
    fn add_assign(&mut self, rhs: FourVector) {
        for i in 0..Self::len() {
            self.p[i] += rhs[i]
        }
        self.update_pt();
    }
}

impl std::ops::SubAssign for FourVector {
    fn sub_assign(&mut self, rhs: FourVector) {
        for i in 0..Self::len() {
            self.p[i] -= rhs[i]
        }
        self.update_pt();
    }
}

impl std::ops::Add for FourVector {
    type Output = Self;

    fn add(mut self, rhs: FourVector) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::Sub for FourVector {
    type Output = Self;

    fn sub(mut self, rhs: FourVector) -> Self::Output {
        self -= rhs;
        self
    }
}

impl From<PseudoJet> for FourVector {
    fn from(p: PseudoJet) -> Self {
        [p.e(), p.px(), p.py(), p.pz()].into()
    }
}

impl From<FourVector> for PseudoJet {
    fn from(p: FourVector) -> Self {
        (&p).into()
    }
}

impl From<&FourVector> for PseudoJet {
    fn from(p: &FourVector) -> Self {
        [p[0], p[1], p[2], p[3]].into()
    }
}
