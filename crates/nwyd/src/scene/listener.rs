use crate::scene::login_panels::{self, Form, LoginPanels};
use crate::scene::{msgbox, status};
use formats::config::{serverlist, sn};
use ui::Strings;
use ui::draw::Owner;
use ui::input::{Event, EventId};

pub const SERVER_VIEW: &str = "ServerListView";

pub(crate) const SETTINGS_ROW: i64 = -1;

pub const CHANNEL_VIEW: &str = "ServerListView2";

pub const BTN_EXIT: &str = "btnExit";

pub const BTN_SEL_SERVER: &str = "btnSelServer";

pub const BTN_LOGIN: &str = "btnLogin";

pub const BTN_LOGOUT: &str = "btnLogout";

pub const BTN_NEW: &str = "btnNew";

pub const EXIT_STRDEF: usize = 22;

pub const LOGOUT_STRDEF: usize = 11;

pub const NEW_ACCOUNT_STRDEF: (usize, usize) = (9, 10);

pub const FULL: i32 = 900;

pub const FULL_CAPTION: i32 = 0x385;

pub const BAR_DOUBLE_BELOW: i32 = 300;

pub const CAPTION_PAD: usize = 14;

pub const BAR_RANGE: [i32; 2] = [0, 500];

pub const BAR_LAYER: usize = 1;

pub const BAR_RED: u32 = 0xFFFF_0000;

pub const BAR_GREEN: u32 = 0xFF00_FF99;

pub const STR_NUMBERED: usize = 68;

pub const STR_NO_SERVER: usize = 70;

pub const ACCOUNT_SHORT_STRDEF: usize = 3;

pub const PASSWORD_SHORT_STRDEF: usize = 5;

pub const CONNECTING_STRDEF: usize = 7;

pub const CONNECT_FAILED_STRDEF: usize = 8;

pub const NO_SERVER_STRDEF: usize = 24;

pub const SERVER_FULL_STRDEF: usize = 25;

pub const LOGIN_INTERVAL_MS: u32 = 1_500;

pub const ACCOUNT_MIN: usize = 4;

pub const ACCOUNT_MAX: usize = 14;

pub const PASSWORD_MIN: usize = 4;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub server: Option<usize>,
    pub channel: Option<usize>,
    pub address: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picked {
    pub server: usize,
    pub channel: usize,
    pub status: status::Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    NoChannel,
    NoServer,
    ServerFull(i32),
    LoginTooSoon,
    Credentials(&'static str),
    LoginAccepted { account: String, password: String },
    LauncherAccepted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub index: usize,
    pub address: String,
    pub population: i32,
    pub bar: i32,
    pub caption: String,
    pub picked: bool,
    pub crown: bool,
    pub present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bounds {
    pub slot: i32,
    pub last_record: i32,
}

pub fn bounds(names: &sn::ServerNames, table: &serverlist::ServerList) -> Bounds {
    let configured = table
        .servers
        .iter()
        .position(|s| s.status_url.is_empty())
        .unwrap_or(serverlist::SERVERS);

    let highest = names.slots.iter().copied().max().unwrap_or(0);

    let mut slot = (configured as i32 - 1).max(highest);

    if table
        .servers
        .get(3)
        .and_then(|s| s.channels.get(1))
        .is_some_and(|c| !c.is_empty())
    {
        slot = 9;
    }

    Bounds {
        slot,
        last_record: highest.max(0) - 1,
    }
}

pub fn aggregate_record(table: &serverlist::ServerList) -> usize {
    (0..=serverlist::SERVERS)
        .find(|&record| {
            table
                .servers
                .get(record)
                .is_none_or(|server| server.status_url.is_empty())
        })
        .unwrap_or(serverlist::SERVERS)
}

#[derive(Debug, Clone)]
pub struct Tables {
    pub table: serverlist::ServerList,
    pub names: sn::ServerNames,
    pub bounds: Bounds,
}

pub fn recommended(
    table: &serverlist::ServerList,
    status: &status::Status,
    w_day: i32,
) -> [i32; serverlist::SERVERS] {
    let mut out = [0; serverlist::SERVERS];

    for (server, slot) in table.servers.iter().zip(out.iter_mut()) {
        let count = server.channels.iter().filter(|c| !c.is_empty()).count() as i32;

        *slot = status.rotation(count, w_day);
    }

    out
}

pub fn crown(spare: i32, day: i32, epoch_day: i64, channel: usize) -> bool {
    if spare == 1 {
        let week = ((day - 1) / 7 + 1) & 1;

        return (channel as i32 & 1) == week;
    }

    epoch_day.rem_euclid(7) == 0 && ((epoch_day.div_euclid(7)) & 1) != ((channel as i64 - 1) & 1)
}

fn numbered(format: &str, first: i32, second: i32) -> String {
    let mut out = String::with_capacity(format.len() + 8);

    let mut args = [first, second].into_iter();

    let mut chars = format.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' && chars.peek() == Some(&'d') {
            chars.next();

            match args.next() {
                Some(v) => out.push_str(&v.to_string()),
                None => out.push_str("%d"),
            }

            continue;
        }

        out.push(c);
    }

    out
}

pub fn caption(label: &str, channel: usize, population: i32) -> String {
    let mut text = format!("{label}-{channel}");

    if population >= FULL_CAPTION {
        while text.len() < CAPTION_PAD {
            text.push(' ');
        }

        text.push_str("FULL");
    }

    text
}

pub fn bar_value(population: i32) -> i32 {
    let p = population.max(0);

    if p < BAR_DOUBLE_BELOW { p * 2 } else { p }
}

pub struct RowInput<'a> {
    pub table: &'a serverlist::ServerList,
    pub names: &'a sn::ServerNames,
    pub status: &'a status::Status,
    pub strings: &'a Strings,
    pub bounds: Bounds,
    pub w_day: i32,
    pub epoch_day: i64,
}

impl RowInput<'_> {
    fn strdef(&self, record: usize) -> String {
        self.strings
            .strdef
            .as_ref()
            .map(|s| s.get(record).to_string())
            .unwrap_or_default()
    }
}

pub struct AggregateInput<'a> {
    pub table: &'a serverlist::ServerList,
    pub names: &'a sn::ServerNames,
    pub statuses: &'a [Option<status::Status>],
    pub strings: &'a Strings,
    pub w_day: i32,
}

impl AggregateInput<'_> {
    fn strdef(&self, record: usize) -> String {
        self.strings
            .strdef
            .as_ref()
            .map(|s| s.get(record).to_string())
            .unwrap_or_default()
    }
}

fn aggregate_channel(server: &serverlist::Server, w_day: i32) -> usize {
    let count = server
        .channels
        .iter()
        .take(serverlist::SERVERS)
        .filter(|channel| !channel.is_empty())
        .count();

    if count == 0 {
        return 0;
    }

    let picked = w_day % count as i32;

    if picked == 0 { count } else { picked as usize }
}

pub fn aggregate_channels(input: &AggregateInput<'_>, record: usize) -> Option<Vec<Channel>> {
    let source_count = aggregate_record(input.table);

    if record != source_count {
        return None;
    }

    let mut rows = Vec::with_capacity(source_count);

    for row in 1..=source_count {
        let source = source_count - row;

        let Some(server) = input.table.servers.get(source) else {
            continue;
        };

        let channel = aggregate_channel(server, input.w_day);

        let address = channel
            .checked_sub(1)
            .and_then(|index| server.channels.get(index))
            .cloned()
            .unwrap_or_default();

        let population = input
            .statuses
            .get(source)
            .and_then(Option::as_ref)
            .and_then(|status| {
                channel
                    .checked_sub(1)
                    .and_then(|index| status.populations.get(index))
            })
            .copied()
            .unwrap_or(-1);

        if address.is_empty() {
            if row < source_count {
                rows.push(Channel {
                    index: row,
                    address,
                    population,
                    bar: population.max(0),
                    caption: input.strdef(STR_NO_SERVER),
                    picked: false,
                    crown: false,
                    present: false,
                });
            }

            continue;
        }

        let label = input.names.labels.get(source).cloned().unwrap_or_default();

        let caption = if population < 0 {
            input.strdef(STR_NO_SERVER)
        } else {
            caption(&label, channel, population)
        };

        rows.push(Channel {
            index: row,
            address,
            population,
            bar: bar_value(population),
            caption,
            picked: true,
            crown: false,
            present: true,
        });
    }

    Some(rows)
}

pub fn channels(input: &RowInput<'_>, record: usize) -> Vec<Channel> {
    let Some(server) = input.table.servers.get(record) else {
        return Vec::new();
    };

    let rotated = recommended(input.table, input.status, input.w_day);

    let day = if input.status.day == -1 {
        input.w_day
    } else {
        input.status.day
    };

    let last = input.bounds.last_record;

    let on_last = record as i32 == last;

    let mut rows = Vec::new();

    for channel in 1..=serverlist::SERVERS {
        let address = server
            .channels
            .get(channel - 1)
            .cloned()
            .unwrap_or_default();

        let population = input
            .status
            .populations
            .get(channel - 1)
            .copied()
            .unwrap_or(-1);

        if address.is_empty() {
            if !(on_last && (channel as i32) < input.bounds.slot) {
                continue;
            }

            rows.push(Channel {
                index: channel,
                address,
                population,
                bar: population.max(0),
                caption: input.strdef(STR_NO_SERVER),
                picked: false,
                crown: false,
                present: false,
            });

            continue;
        }

        let mut crowned = crown(input.status.spare, day, input.epoch_day, channel);

        let mut bar = population.max(0);

        let mut text = String::new();

        if on_last {
            let derived = last - channel as i32;

            let named = derived >= 0
                && (0..=last).any(|i| input.names.slots.get(i as usize) == Some(&(derived + 1)));

            if named {
                let derived = derived as usize;

                let pick = rotated.get(derived).copied().unwrap_or(0);

                crowned = crown(-1, input.status.day, input.epoch_day, pick.max(0) as usize);

                let label = input.names.labels.get(derived).cloned().unwrap_or_default();

                text = caption(&label, pick.max(0) as usize, population);

                bar = bar_value(population);
            }
        } else {
            let label = input.names.labels.get(record).cloned().unwrap_or_default();

            if label.is_empty() {
                text = numbered(
                    &input.strdef(STR_NUMBERED),
                    record as i32 + 1,
                    channel as i32,
                );
            } else {
                text = caption(&label, channel, population);

                bar = bar_value(population);
            }
        }

        if population < 0 {
            text = input.strdef(STR_NO_SERVER);
        }

        rows.push(Channel {
            index: channel,
            address,
            population,
            bar,
            caption: text,
            picked: channel as i32 == rotated.get(record).copied().unwrap_or(0) || on_last,
            crown: crowned,
            present: true,
        });
    }

    rows
}

pub fn populate_channels(scene: &mut LoginPanels, record: usize) {
    let fetched = scene.status.clone();

    let status = if scene.status_for == Some(record) {
        fetched
    } else {
        status::Status::default()
    };

    let day = if status.day == -1 {
        scene.time.calendar.server_day_of_month as i32
    } else {
        status.day
    };

    scene.aggregate.refresh(&scene.servers, day);

    let rows = channels(
        &RowInput {
            table: &scene.servers,
            names: &scene.names,
            status: &status,
            strings: &scene.strings,
            bounds: scene.bounds,
            w_day: scene.time.calendar.server_day_of_month as i32,
            epoch_day: scene.time.calendar.epoch_day,
        },
        record,
    );

    scene.channels = rows.clone();

    scene.showing = Some(record);

    let Some(list) = scene
        .login
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.name == CHANNEL_VIEW)
        .and_then(|e| e.widget.list_mut())
    else {
        return;
    };

    fill_channel_list(list, &rows, CrownRule::Rotated);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrownRule {
    Rotated,
    Hidden,
}

pub fn fill_channel_list(list: &mut ui::widget::ListState, rows: &[Channel], crown: CrownRule) {
    list.clear_items();

    for channel in rows {
        let index = list.add_item("");

        let row = &mut list.items[index];

        row.payload = channel.bar as i64;

        for child in &mut row.children {
            match child.name.as_str() {
                "Name" => child.set_display(&channel.caption),
                "Bar" if !channel.present => child.visible = false,
                "Bar" => {
                    if let Some(gauge) = child.widget.gauge_mut() {
                        gauge.position = channel.bar;

                        gauge.range = BAR_RANGE;
                    }

                    let color = if channel.picked { BAR_GREEN } else { BAR_RED };

                    child.set_layer_texture_color(BAR_LAYER, color);
                }
                "Crown" => child.visible = crown == CrownRule::Rotated && channel.crown,
                "Age" | "Devil" => child.visible = false,
                _ => {}
            }
        }
    }
}

fn selection(scene: &LoginPanels, view: &str) -> Option<usize> {
    let list = scene
        .login
        .ui
        .panel
        .elements
        .iter()
        .find(|e| e.name == view)?
        .widget
        .list()?;

    (list.selected >= 0).then_some(list.selected as usize)
}

fn record_of_row(scene: &LoginPanels, row: usize) -> Option<usize> {
    let list = scene
        .login
        .ui
        .panel
        .elements
        .iter()
        .find(|e| e.name == SERVER_VIEW)?
        .widget
        .list()?;

    let slot = list.items.get(row)?.payload;

    let value = *scene.names.slots.get(usize::try_from(slot).ok()?)?;

    (value >= 1).then_some(value as usize - 1)
}

fn requests_settings(panel: &ui::Panel, event: &Event) -> bool {
    let Some(list) = panel
        .elements
        .iter()
        .find(|element| element.name == SERVER_VIEW)
        .and_then(|element| element.widget.list())
    else {
        return false;
    };

    let row = match event.id.command_id() {
        EventId::ListSelect | EventId::ListDoubleSelect if event.name == SERVER_VIEW => event.extra,
        EventId::Click if event.name == BTN_SEL_SERVER => i64::from(list.selected),
        _ => return false,
    };

    usize::try_from(row)
        .ok()
        .and_then(|index| list.items.get(index))
        .is_some_and(|row| row.payload == SETTINGS_ROW)
}

fn select_server(scene: &mut LoginPanels) -> Result<(), Refusal> {
    let channel_row = selection(scene, CHANNEL_VIEW).ok_or(Refusal::NoChannel)?;

    let channel = scene
        .channels
        .get(channel_row)
        .cloned()
        .ok_or(Refusal::NoChannel)?;

    let server_row = selection(scene, SERVER_VIEW).ok_or(Refusal::NoServer)?;

    let record = record_of_row(scene, server_row).ok_or(Refusal::NoServer)?;

    if channel.bar as u32 >= 900 {
        return Err(Refusal::ServerFull(channel.bar));
    }

    scene.session = Session {
        server: Some(record),
        channel: Some(channel.index),
        address: channel.address,
    };

    scene.login.ui.panel.visible = false;

    scene.login2.ui.panel.visible = !scene.launcher_handoff;

    scene.form = Form::Login;

    if !scene.launcher_handoff {
        scene.focus_login_field();
    }

    Ok(())
}

fn login(scene: &LoginPanels, now_ms: u32) -> Refusal {
    if now_ms < scene.last_login_ms.saturating_add(LOGIN_INTERVAL_MS) {
        return Refusal::LoginTooSoon;
    }

    let account = scene.field_text(login_panels::FIELD_ACCOUNT);

    let password = scene.field_text(login_panels::FIELD_PASSWORD);

    if account.chars().count() < ACCOUNT_MIN {
        return Refusal::Credentials("the account is shorter than 4 characters");
    }

    if account.chars().count() > ACCOUNT_MAX {
        return Refusal::Credentials("the account is longer than 14 characters");
    }

    if password.chars().count() < PASSWORD_MIN {
        return Refusal::Credentials("the password is shorter than 4 characters");
    }

    if password.chars().count() > 11 {
        return Refusal::Credentials("the password is longer than 11 characters");
    }

    Refusal::LoginAccepted { account, password }
}

pub fn handle(scene: &mut LoginPanels, events: &[Event], now_ms: u32) -> Vec<Refusal> {
    let mut refusals = Vec::new();

    for event in events {
        if requests_settings(&scene.login.ui.panel, event) {
            scene.settings_requested = true;

            scene.channels.clear();

            scene.showing = None;

            clear_channel_view(scene);

            continue;
        }

        match event.id.command_id() {
            EventId::ListSelect if event.name == SERVER_VIEW => {
                match usize::try_from(event.extra)
                    .ok()
                    .and_then(|row| record_of_row(scene, row))
                {
                    Some(record) => {
                        crate::scene::login_panels::request_status(scene, record);

                        populate_channels(scene, record);
                    }
                    None => {
                        scene.channels.clear();

                        scene.showing = None;

                        clear_channel_view(scene);
                    }
                }
            }
            EventId::Click => match event.name.as_str() {
                BTN_SEL_SERVER => {
                    if let Err(refusal) = select_server(scene) {
                        if refusal == Refusal::NoServer {
                            scene
                                .message_panel
                                .post(NO_SERVER_STRDEF, 4000, now_ms, true);
                        } else if matches!(refusal, Refusal::ServerFull(_)) {
                            let text = scene.strings.translate(&format!("ss_{SERVER_FULL_STRDEF}"));

                            scene.notice.post(&text, 4000, true, now_ms);
                        }

                        refusals.push(refusal);
                    } else if scene.launcher_handoff {
                        scene
                            .notice
                            .post(&scene.strings.translate("ss_7"), 4000, false, now_ms);

                        refusals.push(Refusal::LauncherAccepted);
                    }
                }
                BTN_LOGIN => {
                    let refusal = login(scene, now_ms);

                    if matches!(&refusal, Refusal::LoginAccepted { .. }) {
                        scene.login_started(now_ms);
                    }

                    let record = match &refusal {
                        Refusal::Credentials(_)
                            if scene
                                .field_text(login_panels::FIELD_ACCOUNT)
                                .chars()
                                .count()
                                < ACCOUNT_MIN =>
                        {
                            Some(3)
                        }
                        Refusal::Credentials(_)
                            if scene
                                .field_text(login_panels::FIELD_ACCOUNT)
                                .chars()
                                .count()
                                > ACCOUNT_MAX =>
                        {
                            Some(4)
                        }
                        Refusal::Credentials(_)
                            if scene
                                .field_text(login_panels::FIELD_PASSWORD)
                                .chars()
                                .count()
                                > 11 =>
                        {
                            Some(6)
                        }
                        Refusal::Credentials(_) => Some(5),
                        Refusal::LoginAccepted { .. } => Some(7),
                        _ => None,
                    };

                    if let Some(record) = record {
                        let text = scene.strings.translate(&format!("ss_{record}"));

                        scene.notice.post(&text, 4000, record != 7, now_ms);
                    }

                    if refusal != Refusal::LoginTooSoon {
                        scene.last_login_ms = now_ms;
                    }

                    refusals.push(refusal);
                }
                BTN_EXIT => raise(
                    scene,
                    msgbox::CONTEXT_EXIT,
                    msgbox::Body::StrDef(EXIT_STRDEF),
                ),
                BTN_LOGOUT => raise(
                    scene,
                    msgbox::CONTEXT_EXIT,
                    msgbox::Body::StrDef(LOGOUT_STRDEF),
                ),
                BTN_NEW => raise(
                    scene,
                    msgbox::CONTEXT_NEW_ACCOUNT,
                    msgbox::Body::TwoLines(NEW_ACCOUNT_STRDEF.0, NEW_ACCOUNT_STRDEF.1),
                ),
                _ => {}
            },
            _ => {}
        }

        let _ = event.owner.map(|o| matches!(o, Owner::Element(_)));
    }

    refusals
}

fn raise(scene: &mut LoginPanels, context: u32, body: msgbox::Body) {
    let request = msgbox::Request::server_select(context, body);

    scene.raise_box(&request);
}

fn clear_channel_view(scene: &mut LoginPanels) {
    if let Some(list) = scene
        .login
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.name == CHANNEL_VIEW)
        .and_then(|e| e.widget.list_mut())
    {
        list.clear_items();
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use formats::config::config_bin::Language;
    use formats::ui::pane::{ElementType, font_align};
    use ui::draw::Interaction;
    use ui::input::{Manager, Mouse, PanelMut};
    use ui::material::Material;

    fn server_panel() -> (ui::Panel, sn::ServerNames, serverlist::ServerList) {
        let resource = ui::Resource::default();

        let mut element =
            ui::Element::new(ElementType::ListView, "ListView", SERVER_VIEW, &resource);

        element.width = 130;

        element.height = 160;

        let list = element.widget.list_mut().unwrap();

        list.item_size = [128, 18];

        list.border_width = 0;

        list.spacing = 0;

        list.item_spacing = 0;

        list.scrollbar_width = 0;

        let mut name = ui::Element::new(ElementType::Static, "Static", "Name", &resource);

        name.width = 120;

        name.height = 18;

        list.template.push(name);

        let panel = ui::Panel {
            right: 180,
            bottom: 180,
            elements: vec![element],
            ..Default::default()
        };

        let mut names = sn::ServerNames {
            labels: std::array::from_fn(|_| String::new()),
            slots: [0; sn::SLOTS],
        };

        names.labels[0] = "Settings".into();

        names.labels[1] = "Other server".into();

        names.slots[0] = 1;

        names.slots[1] = 2;

        let table = serverlist::ServerList {
            servers: (0..2)
                .map(|index| serverlist::Server {
                    status_url: format!("https://example.invalid/status/{index}"),
                    channels: Vec::new(),
                })
                .collect(),
            ..Default::default()
        };

        (panel, names, table)
    }

    fn event(id: EventId, name: &str, extra: i64) -> Event {
        Event {
            panel: 0,
            owner: None,
            name: name.into(),
            extra,
            id,
        }
    }

    #[test]
    fn settings_row_is_localized_and_remains_last_when_repopulated() {
        let (mut panel, names, table) = server_panel();

        let original_bounds = bounds(&names, &table);

        for (language, label) in [
            (Language::English, "Settings"),
            (Language::Portuguese, "Configurações"),
            (Language::Korean, "Settings"),
        ] {
            login_panels::populate_server_list(
                &mut panel,
                &names,
                &table,
                &Strings::default(),
                language,
            );

            let rows = &panel.elements[0].widget.list().unwrap().items;

            assert_eq!(rows.len(), 3);

            assert_eq!(rows[0].payload, 1);

            assert_eq!(rows[0].children[0].display, "Other server");

            assert_eq!(rows[1].payload, 0);

            assert_eq!(rows[1].children[0].display, "Settings");

            assert_eq!(rows[2].payload, SETTINGS_ROW);

            assert!(rows[2].text.is_empty());

            assert_eq!(rows[2].children[0].display, label);

            assert_eq!(bounds(&names, &table), original_bounds);
        }
    }

    #[test]
    fn settings_row_draws_one_template_label_in_each_language() {
        for (language, label) in [
            (Language::English, "Settings"),
            (Language::Portuguese, "Configurações"),
        ] {
            let (mut panel, mut names, table) = server_panel();

            names.labels[0] = "First server".into();

            let element = &mut panel.elements[0];

            element.layers = vec![
                Material {
                    font_align: font_align::VCENTER,
                    ..Material::seed()
                },
                Material::seed(),
            ];

            let name = &mut element.widget.list_mut().unwrap().template[0];

            name.x = 2;

            name.y = 1;

            name.height = 16;

            name.layers = vec![Material {
                font_align: font_align::CENTER | font_align::VCENTER,
                ..Material::seed()
            }];

            login_panels::populate_server_list(
                &mut panel,
                &names,
                &table,
                &Strings::default(),
                language,
            );

            for selected in [-1, 2] {
                panel.elements[0].widget.list_mut().unwrap().selected = selected;

                let draws = panel.draw_list(&Interaction::default());

                let settings: Vec<_> = draws.texts().filter(|text| text.text == label).collect();

                assert_eq!(settings.len(), 1, "{language:?}, selected={selected}");

                let settings = settings[0];

                assert_eq!(
                    settings.owner,
                    Owner::ListRow {
                        element: 0,
                        row: 2,
                        child: Some(0),
                    }
                );

                assert_eq!(settings.align, font_align::CENTER | font_align::VCENTER);

                let server = draws
                    .texts()
                    .find(|text| text.text == "Other server")
                    .unwrap();

                assert_eq!(settings.align, server.align);

                assert_eq!(settings.font, server.font);

                assert_eq!(settings.color, server.color);

                assert_eq!(
                    settings.rect,
                    [
                        server.rect[0],
                        server.rect[1] + 36,
                        server.rect[2],
                        server.rect[3] + 36,
                    ]
                );

                assert_eq!(draws.texts().count(), 3);
            }
        }
    }

    #[test]
    fn settings_row_exists_without_configured_servers() {
        let (mut panel, mut names, _) = server_panel();

        names.slots = [0; sn::SLOTS];

        login_panels::populate_server_list(
            &mut panel,
            &names,
            &serverlist::ServerList::default(),
            &Strings::default(),
            Language::Portuguese,
        );

        let rows = &panel.elements[0].widget.list().unwrap().items;

        assert_eq!(rows.len(), 1);

        assert_eq!(rows[0].payload, SETTINGS_ROW);

        assert_eq!(rows[0].children[0].display, "Configurações");
    }

    #[test]
    fn settings_row_pointer_and_confirm_events_request_settings() {
        let (mut panel, names, table) = server_panel();

        login_panels::populate_server_list(
            &mut panel,
            &names,
            &table,
            &Strings::default(),
            Language::English,
        );

        let mut manager = Manager::default();

        let mut origin = (0, 0);

        for mouse in [Mouse::LeftDown, Mouse::LeftUp, Mouse::LeftDouble] {
            let events = manager.mouse(
                &mut [PanelMut {
                    panel: &mut panel,
                    origin: &mut origin,
                }],
                mouse,
                10,
                45,
            );

            if mouse != Mouse::LeftUp {
                assert!(events.iter().any(|event| requests_settings(&panel, event)));
            }
        }

        assert!(requests_settings(
            &panel,
            &event(EventId::Click, BTN_SEL_SERVER, 0)
        ));

        manager.capture = None;

        manager.pressed = None;

        let reopened = manager.mouse(
            &mut [PanelMut {
                panel: &mut panel,
                origin: &mut origin,
            }],
            Mouse::LeftDown,
            10,
            45,
        );

        assert!(
            reopened
                .iter()
                .any(|event| requests_settings(&panel, event))
        );

        for row in [-1, 0, 1, 3, i64::MAX] {
            for id in [EventId::ListSelect, EventId::ListDoubleSelect] {
                assert!(!requests_settings(&panel, &event(id, SERVER_VIEW, row)));
            }
        }

        for name in [CHANNEL_VIEW, BTN_LOGIN, BTN_EXIT] {
            assert!(!requests_settings(
                &panel,
                &event(EventId::ListSelect, name, 2)
            ));
        }

        panel.elements[0].widget.list_mut().unwrap().selected = 1;

        assert!(!requests_settings(
            &panel,
            &event(EventId::Click, BTN_SEL_SERVER, 0)
        ));
    }
}
