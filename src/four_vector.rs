use noisy_float::prelude::*;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Copy, Default)]
pub struct FourVector([N64; 4]);

impl FourVector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn euclid_norm(&self) -> N64 {
        self.euclid_norm_sq().sqrt()
    }

    pub fn euclid_norm_sq(&self) -> N64 {
        self.0.iter().map(|e| *e * *e).sum()
    }

    pub fn spatial_norm(&self) -> N64 {
        self.spatial_norm_sq().sqrt()
    }

    pub fn spatial_norm_sq(&self) -> N64 {
        self.0.iter().skip(1).map(|e| *e * *e).sum()
    }

    const fn len() -> usize {
        4
    }
}

impl std::convert::From<[N64; 4]> for FourVector {
    fn from(comp: [N64; 4]) -> FourVector {
        FourVector(comp)
    }
}

impl std::ops::Index<usize> for FourVector {
    type Output = N64;

    fn index(&self, i: usize) -> &Self::Output {
        &self.0[i]
    }
}

impl std::ops::IndexMut<usize> for FourVector {
    fn index_mut(&mut self, i: usize) -> &mut N64 {
        &mut self.0[i]
    }
}

impl std::ops::AddAssign for FourVector {
    fn add_assign(&mut self, rhs: FourVector) {
        for i in 0..Self::len() {
            self[i] += rhs[i]
        }
    }
}

impl std::ops::SubAssign for FourVector {
    fn sub_assign(&mut self, rhs: FourVector) {
        for i in 0..Self::len() {
            self[i] -= rhs[i]
        }
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
