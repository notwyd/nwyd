#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lcg {
    holdrand: u32,
}

pub const RAND_MASK: u32 = 0x7FFF;

pub const MULTIPLIER: u32 = 0x343FD;

pub const INCREMENT: u32 = 0x269EC3;

impl Lcg {
    pub fn new(seed: u32) -> Lcg {
        Lcg { holdrand: seed }
    }

    pub fn for_object(record: usize, instance: usize) -> Lcg {
        Lcg::new(
            (record as u32)
                .wrapping_mul(16)
                .wrapping_add(instance as u32)
                .wrapping_add(1),
        )
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u32 {
        self.holdrand = self
            .holdrand
            .wrapping_mul(MULTIPLIER)
            .wrapping_add(INCREMENT);

        (self.holdrand >> 16) & RAND_MASK
    }

    pub fn next_mod(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}
