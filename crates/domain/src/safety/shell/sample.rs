pub(crate) struct Random(pub(crate) u64);

impl Random {
    pub(crate) fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub(crate) fn below(&mut self, most: usize) -> usize {
        usize::try_from(self.next() % most.max(1) as u64).unwrap_or_default()
    }

    pub(crate) fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
        from[self.below(from.len())]
    }

    pub(crate) fn chance(&mut self, one_in: usize) -> bool {
        self.below(one_in) == 0
    }
}

pub(crate) fn cases() -> usize {
    std::env::var("SHELL_PROPERTY_CASES").ok().and_then(|cases| cases.parse().ok()).unwrap_or(2_000)
}
