macro_rules! timestamp {
    ($name:ident, $doc:literal) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            pub const fn from_millis(value: u32) -> Self {
                Self(value)
            }

            pub const fn as_millis(self) -> u32 {
                self.0
            }

            pub const fn elapsed_since(self, earlier: Self) -> u32 {
                self.0.wrapping_sub(earlier.0)
            }
        }
    };
}

timestamp!(
    RawTime,
    "Raw OS tick (Windows uptime), unaffected by server synchronization or scene skew."
);

timestamp!(
    GameTime,
    "Synchronized scene time including TimerManager's signed teardown offset."
);

timestamp!(
    WireTime,
    "Synchronized packet timestamp, excluding TimerManager's scene-only offset."
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSample {
    pub raw: RawTime,
    pub game: GameTime,
    pub wire: WireTime,
}
