use super::*;
use crate::clock::Calendar;
use formats::ui::pane::ElementType;

pub(crate) fn placed(elements: &[(&str, ElementType)]) -> PlacedPanel {
    let resource = ui::Resource::default();

    let panel = ui::Panel {
        right: 300,
        bottom: 200,
        elements: elements
            .iter()
            .map(|(name, kind)| ui::Element::new(*kind, "Synthetic", name, &resource))
            .collect(),
        ..Default::default()
    };

    PlacedPanel {
        ui: PanelUi::from_resource(panel, resource, &Default::default(), 1.0),
        origin: (0, 0),
    }
}

pub(crate) fn background() -> (
    u32,
    formats::texture::texture_set_list::Item,
    login_static::LoadedTexture,
) {
    (
        0,
        formats::texture::texture_set_list::Item {
            texture_index: 0,
            x: 0,
            y: 0,
            w: 1,
            h: 1,
            dst_offset: [0, 0],
        },
        login_static::LoadedTexture {
            slot: 0,
            stored_path: "synthetic".into(),
            flag: formats::texture::texture_list::FormatFlag::None_,
            alpha: render::texture::AlphaPolicy::Stored,
            color_key: render::texture::ColorKey::CannotFire,
            keyed_texels: 0,
            texture: formats::texture::Texture {
                width: 1,
                height: 1,
                format: formats::texture::PixelFormat::Rgba8Unorm,
                mips: Vec::new(),
                data: vec![0; 4],
                source_len: 0,
            },
        },
    )
}

pub(crate) fn login() -> LoginPanels {
    let layout = Layout::new(800, 600);

    let time = SceneTime::pinned(2500, Calendar::GOLDEN);

    let mut login = placed(&[
        (listener::SERVER_VIEW, ElementType::ListView),
        (listener::CHANNEL_VIEW, ElementType::ListView),
    ]);

    for element in &mut login.ui.panel.elements {
        let list = element.widget.list_mut().unwrap();

        list.template.push(ui::Element::new(
            ElementType::Static,
            "Static",
            "Name",
            &login.ui.resource,
        ));
    }

    let mut login2 = placed(&[
        (FIELD_ACCOUNT, ElementType::ImeEditBox),
        (FIELD_PASSWORD, ElementType::EditBox),
        (listener::BTN_LOGIN, ElementType::Button),
    ]);

    let login_fields = [
        login2.ui.panel.elements[0].instance,
        login2.ui.panel.elements[1].instance,
    ];

    initialize_credentials(&mut login2.ui.panel, login_fields);

    let login_button = login2.ui.panel.elements[2].instance;

    let mut names = sn::ServerNames {
        labels: std::array::from_fn(|_| String::new()),
        slots: [0; sn::SLOTS],
    };

    names.labels[0] = "Synthetic server".into();

    names.slots[0] = 1;

    let servers = serverlist::ServerList {
        servers: vec![serverlist::Server {
            status_url: "https://example.invalid/status".into(),
            channels: vec!["192.0.2.1:8281".into()],
        }],
        ..Default::default()
    };

    populate_server_list(
        &mut login.ui.panel,
        &names,
        &servers,
        &Strings::default(),
        Language::English,
    );

    LoginPanels {
        legacy_boxes: super::super::legacy_message_box::Dialogs::for_test(),
        notice: super::super::notice::Overlay::for_test(),
        message_panel: super::super::message_panel::Overlay::for_test(),
        layout,
        time,
        s_controls: LoginScene {
            layout,
            time,
            draws: Vec::new(),
            textures: Default::default(),
            missing: Vec::new(),
            logo_width: None,
        },
        login,
        login2,
        login_fields,
        login_button,
        login_attempt_ms: 2310,
        copyright: placed(&[]),
        form: Form::Login,
        captions: true,
        guimat_textures: Default::default(),
        missing: Vec::new(),
        notes: Vec::new(),
        manager: Manager::default(),
        bounds: listener::bounds(&names, &servers),
        aggregate: aggregate::State::new(&servers, 1),
        names,
        servers,
        strings: Strings::default(),
        status: status::Status::default(),
        channels: Vec::new(),
        session: listener::Session::default(),
        last_login_ms: 2100,
        clipboard: Default::default(),
        status_timeout: None,
        status_rx: None,
        status_tx: None,
        showing: None,
        status_for: None,
        status_requested: Vec::new(),
        boxes: msgbox::Boxes::new(),
        msgbox: msgbox::BoxPanel::for_test(),
        box_manager: Manager::default(),
        quit_requested: false,
        settings_requested: false,
        registration_requests: Vec::new(),
        launcher_login: false,
        launcher_handoff: false,
    }
}

#[test]
fn characterization_login_resize_preserves_selection_status_credentials_and_dialogs() {
    let mut scene = login();

    scene.login.ui.panel.elements[0]
        .widget
        .list_mut()
        .unwrap()
        .selected = 0;

    scene.status.populations[0] = 123;

    scene.showing = Some(0);

    scene.status_for = Some(0);

    scene.session = listener::Session {
        server: Some(0),
        channel: Some(0),
        address: "192.0.2.1:8281".into(),
    };

    scene
        .status_requested
        .push((0, "https://example.invalid/status".into()));

    for (index, text) in ["synthetic_user", "test_secret"].into_iter().enumerate() {
        let edit = scene.login2.ui.panel.elements[index]
            .widget
            .edit_mut()
            .unwrap();

        edit.text = text.into();

        edit.caret = 3;

        edit.anchor = 1;
    }

    scene.login_started(2300);

    scene.legacy_boxes.show(
        super::super::legacy_message_box::Owner::Question,
        "Existing question",
        42,
        None,
    );

    scene.notice.post("Existing notice", 4000, true, 2500);

    scene.message_panel.post(12, 4000, 2500, true);

    let edits = [0, 1].map(|index| {
        scene.login2.ui.panel.elements[index]
            .widget
            .edit()
            .unwrap()
            .clone()
    });

    let status = scene.status.clone();

    let session = scene.session.clone();

    let dialogs = scene.legacy_boxes.states.clone();

    let notice = scene.notice.state.clone();

    let message = scene.message_panel.state.clone();

    scene.resize(&Default::default(), (800, 600));

    assert_eq!(
        scene.login.ui.panel.elements[0]
            .widget
            .list()
            .unwrap()
            .selected,
        0
    );

    for (index, edit) in edits.iter().enumerate() {
        assert_eq!(
            scene.login2.ui.panel.elements[index].widget.edit().unwrap(),
            edit
        );
    }

    assert_eq!(scene.status, status);

    assert_eq!(scene.session, session);

    assert_eq!(scene.showing, Some(0));

    assert_eq!(scene.status_for, Some(0));

    assert_eq!(scene.status_requested.len(), 1);

    assert_eq!(scene.last_login_ms, 2100);

    assert_eq!(scene.login_attempt_ms, 2300);

    assert_eq!(scene.legacy_boxes.states, dialogs);

    assert_eq!(scene.notice.state, notice);

    assert_eq!(scene.message_panel.state, message);

    assert!(!scene.login2.ui.panel.elements[1].enabled);

    assert!(!scene.login2.ui.panel.elements[2].enabled);
}

pub(crate) struct LanguageGame {
    root: std::path::PathBuf,
    pub game: GameDir,
}

impl LanguageGame {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

        let root = std::env::temp_dir().join(format!(
            "nwyd-scene-language-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));

        std::fs::create_dir_all(&root).unwrap();

        let game = GameDir::open(&root).unwrap();

        Self { root, game }
    }
}

impl Drop for LanguageGame {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub(crate) fn translated(records: &[(usize, &str)], ui: &str) -> Strings {
    let mut raw = vec![0; formats::text::strdef::LEN];

    for &(record, text) in records {
        let bytes = formats::text::codepage::encode(text, Codepage::Windows1252);

        let start = record * formats::text::strdef::RECORD_LEN;

        raw[start..start + bytes.len()].copy_from_slice(&bytes);
    }

    let checksum = formats::text::strdef::checksum(&raw);

    raw.extend_from_slice(&checksum.wrapping_add(1).to_le_bytes());

    Strings::new(
        formats::text::string_table::ui::parse(ui.as_bytes(), Codepage::Windows1252),
        formats::text::strdef::parse(&raw, Codepage::Windows1252),
    )
}

#[test]
fn runtime_language_preserves_login_network_rows_edits_and_dialogs() {
    let mut fixture = LanguageGame::new();

    let mut scene = login();

    scene.names.labels[0].clear();

    scene.strings = translated(
        &[(66, "Server %d"), (listener::STR_NO_SERVER, "Unavailable")],
        "",
    );

    scene.status.populations[0] = -1;

    scene.status_for = Some(0);

    listener::populate_channels(&mut scene, 0);

    for element in &mut scene.login.ui.panel.elements {
        element.widget.list_mut().unwrap().selected = 0;
    }

    for (index, text) in ["synthetic_user", "test_secret"].into_iter().enumerate() {
        let edit = scene.login2.ui.panel.elements[index]
            .widget
            .edit_mut()
            .unwrap();

        edit.text = text.into();

        edit.caret = 3;

        edit.anchor = 1;
    }

    scene.login_started(2300);

    scene.session.address = "192.0.2.1:8281".into();

    scene
        .status_requested
        .push((0, "https://example.invalid/status".into()));

    scene.legacy_boxes.show(
        super::super::legacy_message_box::Owner::Question,
        "Existing question",
        42,
        None,
    );

    scene.notice.post("Existing notice", 4000, true, 2500);

    let edits: Vec<_> = scene
        .login2
        .ui
        .panel
        .elements
        .iter()
        .filter_map(|e| e.widget.edit().cloned())
        .collect();

    let ids: Vec<_> = scene
        .login
        .ui
        .panel
        .elements
        .iter()
        .flat_map(|e| &e.widget.list().unwrap().items)
        .flat_map(|row| &row.children)
        .map(|child| child.instance)
        .collect();

    let status = scene.status.clone();

    let session = scene.session.clone();

    let channels = scene.channels.clone();

    let dialogs = scene.legacy_boxes.states.clone();

    let notice = scene.notice.state.clone();

    for (language, server, unavailable, settings) in [
        (
            Language::Portuguese,
            "Servidor %d",
            "Indisponível",
            "Configurações",
        ),
        (Language::English, "Server %d", "Unavailable", "Settings"),
    ] {
        fixture.game.set_language(language);

        let strings = translated(&[(66, server), (listener::STR_NO_SERVER, unavailable)], "");

        let update = LanguageUpdate::prepare(&scene, &fixture.game, &strings).unwrap();

        scene.apply_language(update);

        let list = scene.login.ui.panel.elements[0].widget.list().unwrap();

        assert_eq!(list.selected, 0);

        assert_eq!(list.items[0].children[0].display, server.replace("%d", "1"));

        assert_eq!(list.items.last().unwrap().children[0].display, settings);

        assert!(list.items.iter().all(|row| row.text.is_empty()));

        let channel_list = scene.login.ui.panel.elements[1].widget.list().unwrap();

        assert_eq!(channel_list.selected, 0);

        assert_eq!(channel_list.items[0].children[0].display, unavailable);

        let mut expected_channels = channels.clone();

        for channel in &mut expected_channels {
            channel.caption = unavailable.into();
        }

        assert_eq!(scene.channels, expected_channels);

        let current_ids: Vec<_> = scene
            .login
            .ui
            .panel
            .elements
            .iter()
            .flat_map(|e| &e.widget.list().unwrap().items)
            .flat_map(|row| &row.children)
            .map(|child| child.instance)
            .collect();

        assert_eq!(current_ids, ids);

        assert_eq!(scene.status, status);

        assert_eq!(scene.session, session);

        assert_eq!(scene.status_requested.len(), 1);

        assert_eq!(scene.showing, Some(0));

        assert_eq!(scene.status_for, Some(0));

        assert_eq!(scene.login_attempt_ms, 2300);

        assert_eq!(scene.last_login_ms, 2100);

        assert_eq!(scene.legacy_boxes.states, dialogs);

        assert_eq!(scene.notice.state, notice);

        for (index, edit) in edits.iter().enumerate() {
            assert_eq!(
                scene.login2.ui.panel.elements[index].widget.edit().unwrap(),
                edit
            );
        }

        assert!(!scene.login2.ui.panel.elements[1].enabled);

        assert!(!scene.login2.ui.panel.elements[2].enabled);
    }
}
