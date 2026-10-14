use super::dispatch::BatchEffects;
use crate::scene::{Scene, item_prices, shop, store, trade};
use assets::GameDir;
use formats::net::{autotrade, item_prices::Update, shop as wire_shop, trade::Offer};

pub enum Packet {
    Prices(Update),
    ShopClosed,
    Sold(wire_shop::Sale),
    Contents(Box<autotrade::Contents>),
    SlotSold(autotrade::Sold),
    Stock(Box<wire_shop::Stock>),
    SellList(Box<wire_shop::SellList>),
    Offer { entity: u16, offer: Offer },
    TradeClosed { entity: u16 },
    Toggle { entity: u16 },
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub prices: &'a mut item_prices::Retained,
    pub game: Option<&'a GameDir>,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn load_panel_icons(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet, effects: &mut BatchEffects) {
    match packet {
        Packet::Prices(update) => {
            let state = host.state();

            state.prices.receive(state.scene, update);
        }
        Packet::ShopClosed => match host.state().scene.shop_closed() {
            Some(true) => {
                tracing::info!("NWYD: 0x196 — the NPC item shop and its bag are shut");

                host.redraw();
            }
            Some(false) => {
                tracing::info!("NWYD: 0x196 — the NPC item shop was not open; the bag is unchanged")
            }
            None => tracing::info!("NWYD: 0x196 arrived with no world up"),
        },
        Packet::Sold(sale) => {
            let state = host.state();

            let Some(game) = state.game else {
                return;
            };

            match state.scene.sold(game, &sale) {
                Some(Ok(cleared)) => {
                    tracing::info!("{}", shop::describe_sale(&cleared));

                    if cleared.redressed {
                        effects.request_upload();
                    }

                    host.redraw();
                }
                Some(Err(error)) => tracing::info!("NWYD: 0x37A — {error:#}"),
                None => tracing::info!("NWYD: 0x37A arrived with no world up"),
            }
        }
        Packet::Contents(contents) => match host.state().scene.shop_contents(&contents) {
            Some(opened) => {
                tracing::info!("{}", store::describe(&opened));

                host.load_panel_icons();

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x397 arrived with no world up"),
        },
        Packet::SlotSold(sold) => match host.state().scene.shop_slot_sold(&sold) {
            Some(sale) => {
                tracing::info!("{}", store::describe_sale(&sale));

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x39B arrived with no world up"),
        },
        Packet::Stock(stock) => match host.state().scene.shop_stock(&stock) {
            Some(opened) => {
                tracing::info!("{}", shop::describe(&opened));

                host.load_panel_icons();

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x17C arrived with no world up"),
        },
        Packet::SellList(list) => match host.state().scene.shop_sell_list(&list) {
            Some(applied) => {
                tracing::info!("{}", shop::describe_sell_list(&applied));

                if applied.drawn != 0 {
                    host.load_panel_icons();
                }

                host.redraw();
            }
            None => tracing::info!("NWYD: 0x3E8 arrived with no world up"),
        },
        Packet::Offer { entity, offer } => match host.state().scene.trade_offer(entity, &offer) {
            Some(applied) => {
                tracing::info!("{}", trade::describe(&applied));

                let raised = applied.raised.as_ref().is_some_and(|r| r.slot.is_some());

                let contents = matches!(applied.arm, Some(trade::Arm::Contents));

                if contents {
                    host.load_panel_icons();
                }

                if raised || contents {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x383 for entity {entity} arrived with no world up")
            }
        },
        Packet::TradeClosed { entity } => match host.state().scene.trade_closed(entity) {
            Some(applied) => {
                tracing::info!("{}", trade::describe_close(&applied));

                if applied.closed_trade || applied.closed_store {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x384 for entity {entity} arrived with no world up")
            }
        },
        Packet::Toggle { entity } => match host.state().scene.trade_toggle_for(entity) {
            Some(applied) => {
                tracing::info!(
                    "NWYD: 0x386 — control 0x269 forced on, mirror untouched (control {})",
                    if applied.had_control {
                        "present"
                    } else {
                        "absent"
                    },
                )
            }
            None => {
                tracing::info!(
                    entity,
                    "NWYD: 0x386 has no live addressed body in the current world"
                )
            }
        },
    }
}
