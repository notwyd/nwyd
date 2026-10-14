pub mod cursor;
pub mod resource;

pub use cursor::{Cursor, decode_cursor, group_cursor};
pub use resource::{RT_CURSOR, RT_GROUP_CURSOR, Resources};

pub const CURSOR_NORMAL: u32 = 0xA2;

pub const CURSOR_ALTERNATE: u32 = 0xA4;
