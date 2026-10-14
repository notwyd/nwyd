use crate::scene::{Scene, enter::Entered};
use anyhow::Result;
use assets::GameDir;
use formats::{data::item_list::ItemList, net::account};

pub trait Screens {
    fn set_backbuffer(&mut self, backbuffer: (u32, u32));

    fn game(&self) -> &GameDir;

    fn game_mut(&mut self) -> &mut GameDir;

    fn items(&self) -> &ItemList;

    fn server_select(&self) -> Result<Scene>;

    fn select_char(&self, block: Option<&account::Block>, lock: u8) -> Result<Scene>;

    fn field(&self, entered: &Entered) -> Result<Scene>;

    fn write_quick_slots(&self, bytes: &[u8]) -> Result<()>;
}
