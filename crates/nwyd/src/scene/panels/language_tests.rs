use super::*;

pub(super) fn input_screen() -> Screen {
    let resource = ui::Resource::default();

    let mut panel = ui::Panel {
        right: 300,
        bottom: 120,
        ..Default::default()
    };

    let mut label = ui::Element::new(ElementType::Static, "Static", "label", &resource);

    label.set_display("Old label");

    let mut edit = ui::Element::new(
        ElementType::ImeEditBox,
        "ImeEditBox",
        "ChatEditBox",
        &resource,
    );

    let state = edit.widget.edit_mut().unwrap();

    state.text = "/Example synthetic unsent draft".into();

    state.caret = 8;

    state.anchor = 3;

    panel.elements = vec![label, edit];

    Screen {
        content: 99,
        class: "synthetic",
        script: chat::CHAT_INPUT,
        placed: PlacedPanel {
            ui: PanelUi::from_resource(panel, resource, &Chains::default(), 1.0),
            origin: (23, 41),
        },
        open: true,
        page: 2,
        page_of: None,
        cape_flag: None,
        combine_widgets: Default::default(),
        mix_recipe_owners: [None; 24],
        inventory_widgets: Default::default(),
        cargo_widgets: Default::default(),
        refine_widgets: Default::default(),
        sheet_widgets: Default::default(),
        guild_member_list: None,
        guild_popup_base: None,
    }
}

pub(super) fn populated() -> Panels {
    let mut panels = Panels::for_test();

    panels.screens.push(input_screen());

    panels.chat.mode = chat::WHISPER;

    panels.chat.whisper = "Synthetic recipient".into();

    panels.chat.records.push_back(chat::Line {
        text: "Synthetic previous message".into(),
        color: 0xFF123456,
        speaker: None,
    });

    panels.chat.history = vec!["Synthetic sent message".into()];

    panels.chat.sent_at = [11, 22, 33, 44];

    panels.chat.outbox = vec![b"synthetic pending chat".to_vec()];

    panels.cooldowns.stamp(0xF, 1000);

    panels.outbox = vec![item_move::Request::Use {
        slot: 3,
        at: (100, 200),
    }];

    panels.staged = vec![item_move::Request::Drink];

    panels.wire = vec![b"synthetic pending action".to_vec()];

    panels.trade_accepted = Some(17);

    panels.inventory_partner = crate::scene::trade::CONTENT;

    panels.inventory_block_equipment = true;

    panels.refine_command_locked = true;

    panels.items.set_price(3, 76543);

    panels
}

pub(super) fn assert_preserved(panels: &Panels) {
    assert_eq!(panels.chat.mode, chat::WHISPER);

    assert_eq!(panels.chat.whisper, "Synthetic recipient");

    assert_eq!(
        panels.chat.records.front().unwrap().text,
        "Synthetic previous message"
    );

    assert_eq!(panels.chat.history, ["Synthetic sent message"]);

    assert_eq!(panels.chat.sent_at, [11, 22, 33, 44]);

    assert_eq!(panels.chat.outbox, [b"synthetic pending chat".to_vec()]);

    assert!(panels.cooldowns.running(0xF, 1200));

    assert_eq!(
        panels.outbox,
        [item_move::Request::Use {
            slot: 3,
            at: (100, 200)
        }]
    );

    assert_eq!(panels.staged, [item_move::Request::Drink]);

    assert_eq!(panels.wire, [b"synthetic pending action".to_vec()]);

    assert_eq!(panels.trade_accepted, Some(17));

    assert_eq!(panels.inventory_partner, crate::scene::trade::CONTENT);

    assert!(panels.inventory_block_equipment);

    assert!(panels.refine_command_locked);

    assert_eq!(panels.items.get(3).unwrap().price, 76543);

    let screen = &panels.screens[0];

    assert!(screen.open);

    assert_eq!(screen.page, 2);

    let edit = screen
        .element("ChatEditBox", ElementType::ImeEditBox)
        .unwrap()
        .widget
        .edit()
        .unwrap();

    assert_eq!(edit.text, "/Example synthetic unsent draft");

    assert_eq!((edit.caret, edit.anchor), (8, 3));
}

#[test]
fn language_characterization_ui_visit_preserves_chat_transactions_and_prices() {
    let mut panels = populated();

    let origin = panels.screens[0].placed.origin;

    let identity = panels.screens[0].placed.ui.identity();

    panels.for_each_ui(&mut |ui| {
        if let Some(label) = ui.panel.element_mut("label", ElementType::Static) {
            label.set_display("Updated label");
        }
    });

    assert_preserved(&panels);

    assert_eq!(panels.screens[0].placed.origin, origin);

    assert_eq!(panels.screens[0].placed.ui.identity(), identity);

    assert_eq!(
        panels.screens[0]
            .element("label", ElementType::Static)
            .unwrap()
            .display,
        "Updated label"
    );
}

#[test]
fn language_characterization_reposition_preserves_active_drafts_and_queues() {
    let mut panels = populated();

    panels.reposition((1024, 768));

    assert_preserved(&panels);
}

impl Panels {
    pub(crate) fn for_test() -> Self {
        Panels {
            now_ms: 0,
            zone_time: Default::default(),
            sealed_records: Default::default(),
            coin_focus_pending: Vec::new(),
            guild_popup_position_pending: false,
            hover_leaves_pending: Vec::new(),
            spend_repeat: spend::Repeat::default(),
            layout: Layout::new(800, 600),
            screens: Vec::new(),
            guimat_textures: BTreeMap::new(),
            hover_textures: BTreeMap::new(),
            hover_missing: Vec::new(),
            missing: Vec::new(),
            notes: Vec::new(),
            captions: true,
            strings: Strings::default(),
            help: Vec::new(),
            notice_requested: false,
            community_requested: false,
            settings_requested: false,
            memo: Vec::new(),
            quests: Vec::new(),
            quest_game: None,
            quest_request_pending: false,
            quest_last_request: 0,
            quest_response_pending: false,
            items: item_list::ItemList::from_plain(
                vec![0; item_list::BODY_LEN],
                formats::text::Codepage::Windows1252,
            ),
            missions: None,
            mixes: None,
            mix_help: crate::scene::mix::HelpTables {
                help: Vec::new(),
                names: Vec::new(),
            },
            refine_sets: Vec::new(),
            icons: item_icon::parse(&[]),
            skills: None,
            icon_sheets: BTreeMap::new(),
            missing_sheets: Vec::new(),
            tooltip: tooltip::Tip::for_test(),
            hover_tip: hover_tip::Tip::for_test(),
            hover_source: None,
            icon_hover_stamp: None,
            icon_hover_rect: None,
            carry: carry::Carry::for_test(),
            live: item_use::Live::default(),
            cooldowns: item_move::Cooldowns::default(),
            master: skill_master::Master::default(),
            skill_master_desc: String::new(),
            potal_pos: Vec::new(),
            outbox: Vec::new(),
            staged: Vec::new(),
            firecracker_slot: None,
            ticket_accepted: None,
            seal_open: None,
            seal_name_slot: None,
            wire: Vec::new(),
            gate_accepted: false,
            summon_accepted: false,
            guild_fame_accepted_ms: None,
            chief_accepted: None,
            field_body_guild: None,
            item_skill_accepted: None,
            guild_application_accepted: false,
            death_return_accepted: false,
            resurrect_accepted: false,
            trade_accepted: None,
            boxes: msgbox::Boxes::new(),
            msgbox: msgbox::BoxPanel::for_test(),
            prompts: [None; msgbox::SLOTS],
            used: Vec::new(),
            skill_casts: Vec::new(),
            shop_clicks: Vec::new(),
            mission_clicks: Vec::new(),
            mission_reset: false,
            mix_clicks: Vec::new(),
            mix: crate::scene::mix::Browser::default(),
            staging: crate::scene::combine::Staging::default(),
            refine: crate::scene::refine::Refine::default(),
            refine_command_locked: false,
            guild: crate::scene::guild_panel::Guild::default(),
            guild_page_rect: None,
            guild_requests: Vec::new(),
            guild_identity: crate::scene::guild_request::Identity::default(),
            guild_live_session: None,
            guild_request_rng: crate::scene::rng::Lcg::new(1),
            guild_host: None,
            guild_ring: None,
            pgt_target: None,
            pgt_trade_requested: false,
            pgt_clear_target: false,
            pgt_toasts: Vec::new(),
            pgt_messages: Vec::new(),
            collection_filter: crate::scene::collection::Filter::default(),
            arena_cost: 0,
            collection_dirty: false,
            cargo_open_pending: false,
            item_use_clear_hover: false,
            inventory_open_pending: false,
            sheet_open_pending: false,
            sheet_live_pending: false,
            unconditional_sounds: Vec::new(),
            inventory_partner: 0,
            inventory_block_equipment: false,
            store_closed_pending: false,
            trade_closed_pending: false,
            field_close_owner_present: true,
            trade_item_clicks: Vec::new(),
            collection_signed_up: false,
            collection_cell: 0,
            collection_icon: None,
            shop_id: 0,
            shop_prices: item_tip::Shop::default(),
            store: None,
            store_owner_opened: None,
            store_price: None,
            client_config: Default::default(),
            guild_proxy: Default::default(),
            coin_action: 0,
            split_stack: None,
            portal: portal::Portal::default(),
            box_entities: [None; msgbox::SLOTS],
            box_cells: [None; msgbox::SLOTS],
            tooltip_calendar: crate::clock::Calendar::GOLDEN,
            tooltip_portuguese: false,
            merge_item: Default::default(),
            box_groupings: [None; msgbox::SLOTS],
            box_combines: [None; msgbox::SLOTS],
            box_skills: [None; msgbox::SLOTS],
            chat: chat::Chat::new(),
            notice: notice::Notice::default(),
            notices: crate::scene::minipopup::Notices::default(),
            summoner: String::new(),
            reserved: Default::default(),
        }
    }
}
