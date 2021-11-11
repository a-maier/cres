use noisy_float::prelude::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Default)]
pub struct FourVector{
    pt: N64,
    p: [N64; 4],
}

impl FourVector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn euclid_norm(&self) -> N64 {
        self.euclid_norm_sq().sqrt()
    }

    pub fn euclid_norm_sq(&self) -> N64 {
        self.p.iter().map(|e| *e * *e).sum()
    }

    pub fn spatial_norm(&self) -> N64 {
        self.spatial_norm_sq().sqrt()
    }

    pub fn spatial_norm_sq(&self) -> N64 {
        self.p.iter().skip(1).map(|e| *e * *e).sum()
    }

    pub fn pt(&self) -> N64 {
        self.pt
    }

    const fn len() -> usize {
        4
    }

    fn update_pt(&mut self) {
        self.pt = (self.p[1] * self.p[1] + self.p[2] * self.p[2]).sqrt();
    }
}

impl std::convert::From<[N64; 4]> for FourVector {
    fn from(p: [N64; 4]) -> FourVector {
        let mut res = FourVector{p, pt: std::default::Default::default()};
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
