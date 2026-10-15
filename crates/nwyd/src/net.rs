use crate::clock::{Clock, GameTime, TimeSync, WireTime};
use crate::runtime::identity::AccountSlot;
use formats::net::{account, enter, live};
use protocol::client::{self, Fingerprint};
use protocol::transport::{self, Connection, Event};
use protocol::{ServerMessage, server};
use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoginStage {
    Credentials,
    Connect,
    Send,
}

#[derive(Debug)]
pub(crate) struct LoginFailure {
    pub stage: LoginStage,
    message: String,
}

impl LoginFailure {
    fn at(stage: LoginStage, message: impl Into<String>) -> Self {
        Self {
            stage,
            message: message.into(),
        }
    }

    pub(crate) fn fixed_launcher_owner(&self, launcher: bool) -> bool {
        launcher && self.stage == LoginStage::Connect
    }
}

impl std::fmt::Display for LoginFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for LoginFailure {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    #[default]
    Quiet,
    Packets,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Offline,
    Authenticating,
    CharacterSelect,
    Entering,
    InGame,
    Redirecting,
}

#[derive(Debug)]
pub enum Transition {
    QuestFlags([u8; 11]),
    ClientConfig(formats::net::client_config::Update),
    ItemPrices(formats::net::item_prices::Update),
    LoginDenied {
        opcode: u16,
    },
    CharacterNotice {
        opcode: u16,
    },
    CharacterDeleteHold(account::DeleteHold),
    SelectionMetadata(account::SelectionMetadata),
    ServerReturnNotice,
    DynamicKey(formats::net::dynamic_key::Update),
    CharacterNameNotice,
    CharacterNameResult(account::NameResult),
    CharacterSlotEffect {
        slot: u32,
    },
    CharacterSelect {
        block: Box<account::Block>,
        account: String,
    },
    CharacterListChanged {
        block: Box<account::Block>,
    },
    AccountLock {
        opcode: u16,
    },
    LeaveWorld {
        client_id: u16,
    },
    EnterWorld(Box<enter::EnterWorld>),
    Message(Box<server::SystemMessage>),
    Chat(Box<server::ChatLine>),
    Spawn {
        opcode: u16,
        entity: Box<formats::net::spawn::Spawn>,
    },
    Affects {
        entity: u16,
        affects: Box<formats::net::affect::Affects>,
    },
    Action {
        opcode: u16,
        entity: u16,
        action: Box<formats::net::action::Action>,
    },
    Remove {
        entity: u16,
    },
    GroundItem(Box<formats::net::ground_item::Drop>),
    GateCommand(formats::net::gate_command::Command),
    Vanish {
        entity: u16,
        vanish: formats::net::live::Vanish,
    },
    Attack {
        opcode: u16,
        attack: Box<formats::net::attack::Attack>,
    },
    Kill(formats::net::kill::Kill),
    ShopStock(Box<formats::net::shop::Stock>),
    ShopContents(Box<formats::net::autotrade::Contents>),
    ShopSlotSold(formats::net::autotrade::Sold),
    ShopClosed,
    CraftingClosed,
    AnimationProbe(formats::net::animation_probe::Request),
    Question(Box<formats::net::question::Question>),
    LongMessage(Box<formats::net::long_message::LongMessage>),
    ReelResult(Box<formats::net::reel::ResultMessage>),
    CloseCargo,
    QuickBar {
        bar: [u8; formats::net::enter::QUICK_SLOTS],
    },
    ItemMoved {
        dst: (u8, u8),
        src: (u8, u8),
    },
    Sold(formats::net::shop::Sale),
    NameMark {
        entity: u16,
        mode: i32,
    },
    GlowBurst {
        entity: u16,
        mode: i32,
    },
    GuildInvite {
        entity: u16,
        invite: formats::net::guild_invite::Invite,
    },
    GuildApplication,
    Challenge {
        entity: u16,
        challenge: formats::net::challenge::Challenge,
    },
    Inventory {
        entity: u16,
        bag: Box<formats::net::inventory::Bag>,
    },
    Collection {
        entity: u16,
        progress: formats::net::collection::Progress,
    },
    WarehouseGold(formats::net::warehouse::Move),
    Weather(formats::net::weather::Forecast),
    EventCountdown(formats::net::event_hud::Countdown),
    EventRemaining(formats::net::event_hud::Remaining),
    EventPacked(formats::net::event_hud::Packed),
    EventTarget(formats::net::event_hud::Target),
    EventScore(formats::net::event_hud::Score),
    SummonRequest(server::SummonRequest),
    SummonCancel,
    PlateLevel(formats::net::plate::LevelChange),
    PlateGuild(formats::net::plate::GuildLine),
    Guild(formats::net::guild::Membership),
    SiegeState(formats::net::siege::Siege),
    WarFlag(formats::net::siege::WarFlag),
    Kickout(formats::net::kickout::Kickout),
    FieldCarpet(formats::net::field_fx::Carpet),
    FieldSound(formats::net::field_fx::Sound),
    FieldLine(Box<formats::net::kept::Line>),
    KeyedRecord(Box<formats::net::kept::Record>),
    ArenaRound(formats::net::arena::Round),
    ArenaKill(Box<formats::net::arena::Kill>),
    ArenaList(Box<formats::net::arena::List>),
    ItemArrived(formats::net::backpack::Arrival),
    ItemCleared(formats::net::backpack::Clear),
    PartyMember {
        opcode: u16,
        member: formats::net::party::Member,
    },
    PartyLeft(formats::net::party::Leave),
    PartyMarks(formats::net::party::Marks),
    ShopSellList(Box<formats::net::shop::SellList>),
    NpcLine {
        entity: u16,
        line: formats::net::npc_line::NpcLine,
    },
    TradeOffer {
        entity: u16,
        offer: formats::net::trade::Offer,
    },
    TradeClosed {
        entity: u16,
    },
    TradeToggle {
        entity: u16,
    },
    Vitals {
        entity: u16,
        vitals: live::Vitals,
    },
    HpDelta {
        entity: u16,
        delta: live::HpDelta,
    },
    Firework {
        entity: u16,
        glyph: formats::net::firework::Glyph,
    },
    Emote {
        entity: u16,
        emote: formats::net::emote::Emote,
    },
    ActorStatus {
        entity: u16,
        score: Box<live::ActorStatus>,
    },
    PlayerProgression {
        entity: u16,
        score: live::PlayerProgression,
    },
    EquipChanged {
        entity: u16,
        change: Box<live::EquipChange>,
    },
    Gold {
        entity: u16,
        gold: live::Gold,
    },
    SlotPatch {
        entity: u16,
        patch: live::SlotPatch,
    },
    Redirect(server::Redirect),
    Disconnected {
        reason: String,
        socket_closed: bool,
    },
    Quit,
}

#[derive(Default)]
pub struct Session {
    pub guild_proxy: std::sync::Arc<std::sync::Mutex<crate::scene::guild_request::ProxyState>>,
    send_state: protocol::send_state::SharedSender,
    pub(crate) analytics: crate::analytics::Analytics,
    pub client_config: std::sync::Arc<formats::net::client_config::Config>,
    server_return_notice: bool,
    connection: Option<Connection>,
    phase: Phase,
    pub account: String,
    guild_account: String,
    pub block: Option<Box<account::Block>>,
    selection_metadata: [[u32; 3]; 4],
    pub warehouse: Option<Box<crate::scene::enter::Warehouse>>,
    pub slot: Option<AccountSlot>,
    pub fingerprint: Option<Fingerprint>,
    pub verbosity: Verbosity,
    last_send_ms: u32,
    last_limited_opcode: u16,
    evolution_tier: i32,
    sync: TimeSync,
    drift: crate::clock_drift::Monitor,
    report_clock_drift: Option<bool>,
    unhandled: std::collections::BTreeSet<u16>,
}

impl Session {
    #[cfg(test)]
    pub(crate) fn connected_for_test(connection: Connection) -> Self {
        Self {
            connection: Some(connection),
            phase: Phase::InGame,
            account: "synthetic-account".into(),
            ..Default::default()
        }
    }

    pub fn selection_metadata(&self) -> &[[u32; 3]; 4] {
        &self.selection_metadata
    }

    pub fn apply_selection_metadata(&mut self, value: account::SelectionMetadata) -> bool {
        let Some(row) = self.selection_metadata.get_mut(value.index as usize) else {
            return false;
        };

        *row = value.words;

        true
    }

    pub(crate) fn retain_server_return_notice(&mut self) {
        self.server_return_notice = true;
    }

    pub(crate) fn take_server_return_notice(&mut self) -> bool {
        std::mem::take(&mut self.server_return_notice)
    }

    pub(crate) fn guild_account(&self) -> &str {
        &self.guild_account
    }

    pub fn with_analytics(mut self, analytics: crate::analytics::Analytics) -> Self {
        self.analytics = analytics;

        self
    }

    pub fn with_clock_reports(mut self, enabled: bool) -> Self {
        self.report_clock_drift = Some(enabled);

        self
    }

    pub fn reset_clock_drift(&mut self) {
        self.drift.reset();
    }

    pub fn observe_clock_drift(
        &mut self,
        field_body: bool,
        sample: Option<crate::clock_drift::Sample>,
        wire_tick: u32,
    ) -> Result<Option<crate::clock_drift::Event>, String> {
        let active = field_body && self.is_connected();

        let event = self.drift.observe(active, sample);

        if let Some(crate::clock_drift::Event::Report { seconds }) = event {
            tracing::warn!(
                seconds,
                reporting = self.report_clock_drift.unwrap_or(true),
                "NWYD: repeated local wall-clock drift detected"
            );

            if self.report_clock_drift.unwrap_or(true) {
                self.send_field(
                    &mut client::clock_drift(seconds),
                    "local clock drift",
                    wire_tick,
                )?;
            }
        } else if let Some(crate::clock_drift::Event::Strike { seconds }) = event {
            tracing::info!(seconds, "NWYD: local wall-clock drift; baseline restarted");
        }

        Ok(event)
    }

    pub fn new(verbosity: Verbosity) -> Session {
        Session {
            verbosity,
            ..Session::default()
        }
    }

    fn login_fingerprint(&self) -> Result<(Fingerprint, &'static str), String> {
        if let Some(pinned) = self.fingerprint {
            return Ok((pinned, "explicit override"));
        }

        let local = protocol::machine::local_fingerprint().map_err(|error| {
            format!(
                "could not read local adapter identity: {error}; use --hwid to supply an adapter"
            )
        })?;

        if !local.native {
            tracing::warn!(
                "NWYD: using a stable portable adapter identity; a Windows adapter GUID is required for native identity parity"
            );
        }

        Ok((
            local.fingerprint,
            if cfg!(target_arch = "wasm32") {
                "browser device identity"
            } else if local.native {
                "native adapter"
            } else {
                "portable adapter"
            },
        ))
    }

    pub fn with_fingerprint(mut self, fingerprint: Option<Fingerprint>) -> Session {
        self.fingerprint = fingerprint;

        self
    }

    pub fn with_profile(mut self, profile: protocol::send_state::Profile) -> Session {
        self.disconnect();

        self.send_state = std::sync::Arc::new(std::sync::Mutex::new(
            protocol::send_state::Sender::new(profile),
        ));

        self
    }

    pub fn version(&self) -> i32 {
        self.send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .profile()
            .login_version()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn is_connected(&self) -> bool {
        self.connection.as_ref().is_some_and(|c| !c.is_closed())
    }

    pub fn describe(&self) -> String {
        match self.connection.as_ref() {
            None => "offline".to_string(),
            Some(connection) => {
                let (sent, received) = connection.counters();

                format!(
                    "{:?} {} — {sent} sent, {received} received{}",
                    self.phase,
                    connection.peer(),
                    match connection.schedule_is_live() {
                        true => "",
                        false => ", random keys",
                    },
                )
            }
        }
    }

    fn log(&self, line: impl std::fmt::Display) {
        tracing::info!("NWYD net: {line}");
    }

    fn log_packet(&self, line: impl std::fmt::Display) {
        if self.verbosity == Verbosity::Packets {
            tracing::info!("NWYD net: {line}");
        }
    }

    pub fn login(
        &mut self,
        address: &str,
        account: &str,
        password: &str,
        seed: u32,
        now_ms: u32,
    ) -> Result<(), String> {
        self.login_staged(address, account, password, seed, now_ms)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn login_staged(
        &mut self,
        address: &str,
        account: &str,
        password: &str,
        seed: u32,
        now_ms: u32,
    ) -> Result<(), LoginFailure> {
        client::check(account, password)
            .map_err(|refusal| LoginFailure::at(LoginStage::Credentials, refusal.to_string()))?;

        self.login_request(
            address,
            client::Login::Manual { account, password },
            seed,
            now_ms,
        )
    }

    pub fn login_launcher(
        &mut self,
        address: &str,
        credentials: &formats::launcher::Credentials,
        seed: u32,
        now_ms: u32,
    ) -> Result<(), String> {
        self.login_launcher_staged(address, credentials, seed, now_ms)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn login_launcher_staged(
        &mut self,
        address: &str,
        credentials: &formats::launcher::Credentials,
        seed: u32,
        now_ms: u32,
    ) -> Result<(), LoginFailure> {
        if !credentials.enabled() {
            return Err(LoginFailure::at(
                LoginStage::Credentials,
                "launcher login is disabled by type zero",
            ));
        }

        self.login_request(
            address,
            client::Login::Launcher {
                account: credentials.account(),
                password: credentials.password(),
            },
            seed,
            now_ms,
        )
    }

    fn login_request(
        &mut self,
        address: &str,
        request: client::Login<'_>,
        seed: u32,
        now_ms: u32,
    ) -> Result<(), LoginFailure> {
        let account = request.account();

        let opcode = request.opcode();

        let identity = match request {
            client::Login::Launcher { .. } => "[launcher credentials redacted]".to_string(),
            _ => format!("{account:?}"),
        };

        let (host, port) = split_port(address);

        self.log(format!("connecting to {host}:{port} as {identity}"));

        self.disconnect();

        let mut connection =
            Connection::connect(host, port, CONNECT_TIMEOUT, seed).map_err(|error| {
                LoginFailure::at(
                    LoginStage::Connect,
                    format!("could not connect to {host}:{port}: {error}"),
                )
            })?;

        #[cfg(not(target_arch = "wasm32"))]
        self.log(format!(
            "connected to {} — greeting sent",
            connection.peer()
        ));

        #[cfg(target_arch = "wasm32")]
        self.log(format!(
            "connecting through browser gateway to {} — greeting queued",
            connection.peer()
        ));

        let (fingerprint, origin) = self
            .login_fingerprint()
            .map_err(|error| LoginFailure::at(LoginStage::Send, error))?;

        let version = self.version();

        connection.send_state = self.send_state.clone();

        let mut body = client::login(&request, &fingerprint, version);

        connection.send(&mut body, now_ms).map_err(|error| {
            LoginFailure::at(
                LoginStage::Send,
                format!("could not send the login packet: {error}"),
            )
        })?;

        self.log(format!(
            "{opcode:#05X} login sent — account {identity}, {} bytes, version {:#010X}, \
             identity source: {origin}",
            body.len(),
            version,
        ));

        self.connection = Some(connection);

        self.account = account.to_string();

        self.guild_account = account.to_ascii_uppercase();

        self.phase = Phase::Authenticating;

        self.last_send_ms = self
            .send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_tick;

        self.block = None;

        self.slot = None;

        Ok(())
    }

    pub fn redirect(
        &mut self,
        address: &str,
        redirect: &server::Redirect,
        now_ms: u32,
    ) -> Result<(), String> {
        let (host, port) = split_port(address);

        self.log(format!(
            "redirecting to {host}:{port} as {:?}",
            redirect.account
        ));

        {
            let Some(connection) = self.connection.as_mut() else {
                return Err(
                    "could not follow the redirect: there is no retained connection".into(),
                );
            };

            connection
                .reconnect(host, port, CONNECT_TIMEOUT)
                .map_err(|error| format!("could not connect to {host}:{port}: {error}"))?;
        }

        self.drift.reset();

        let (fingerprint, origin) = self.login_fingerprint()?;

        let version = self.version();

        let mut body = client::login(
            &client::Login::Redirect {
                account: &redirect.account,
                token: &redirect.token,
            },
            &fingerprint,
            version,
        );

        self.connection
            .as_mut()
            .expect("the retained connection was replaced above")
            .send_with_client_id(&mut body, 0, now_ms)
            .map_err(|error| format!("could not send the redirect login packet: {error}"))?;

        self.last_send_ms = self
            .send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_tick;

        self.phase = Phase::Redirecting;

        self.guild_account = redirect.account.to_ascii_uppercase();

        self.analytics.identify(&self.guild_account);

        self.log(format!(
            "0x20D redirect login sent — account {:?}, {} bytes, version {:#010X}, \
             identity source: {origin}",
            redirect.account,
            body.len(),
            version,
        ));

        Ok(())
    }

    pub fn take_chat_limit_notice(&mut self) -> Option<usize> {
        self.send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take_notice()
    }

    pub fn send(&mut self, body: &mut [u8], what: &str, now_ms: u32) -> Result<(), String> {
        let client_id = self.connection.as_ref().map_or(0, Connection::client_id);

        self.send_to(body, client_id, what, now_ms)
    }

    pub fn send_selection(
        &mut self,
        body: &mut [u8],
        what: &str,
        now_ms: u32,
    ) -> Result<(), String> {
        match protocol::header::opcode_of(body) {
            Some(0x20F | 0x211 | 0x213 | 0x21F | 0x3A0 | 0xFAA | 0xFDE) => {
                self.send_to(body, 0, what, now_ms)
            }
            Some(0x334) => self.send(body, what, now_ms),
            _ => Err(format!(
                "{what} was not sent: not a character-selection packet"
            )),
        }
    }

    fn send_to(
        &mut self,
        body: &mut [u8],
        client_id: u16,
        what: &str,
        now_ms: u32,
    ) -> Result<(), String> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(format!("{what} was not sent: there is no connection"));
        };

        connection
            .send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .config = self.client_config.clone();

        let opcode = protocol::header::opcode_of(body).unwrap_or(0);

        if let Err(error) = connection.send_with_client_id(body, client_id, now_ms) {
            tracing::warn!(
                connection = connection.id(), phase = ? self.phase,
                last_successful_send_tick = self.last_send_ms, attempted_tick = now_ms,
                opcode, % error, "packet send failed"
            );

            return Err(format!("could not send {what}: {error}"));
        }

        self.last_send_ms = connection
            .send_state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .last_tick;

        self.log_packet(format!("→ {opcode:#05x} {what}, {} bytes", body.len()));

        Ok(())
    }

    pub fn send_field(&mut self, body: &mut [u8], what: &str, now_ms: u32) -> Result<bool, String> {
        let opcode = protocol::header::opcode_of(body).unwrap_or(0);

        if opcode == crate::scene::item_move::OPCODE_MOVE {
            tracing::debug!(
                target : "nwyd::body_state", event = "item_move_send_attempt", connection
                = ? self.connection.as_ref().map(Connection::id), slot = ? self.slot,
                now_ms, dst_container = ? body.get(0x0c), dst_slot = ? body.get(0x0d),
                src_container = ? body.get(0x0e), src_slot = ? body.get(0x0f),
                "Item move reached transport"
            );
        }

        if !field_send_allowed(self.last_limited_opcode, opcode, self.last_send_ms, now_ms) {
            self.log_packet(format!("{opcode:#05x} {what} throttled"));

            if opcode == crate::scene::item_move::OPCODE_MOVE {
                tracing::debug!(
                    target : "nwyd::body_state", event = "item_move_throttled",
                    "Item move blocked by send throttle"
                );
            }

            return Ok(false);
        }

        let result = self.send(body, what, now_ms);

        if opcode == crate::scene::item_move::OPCODE_MOVE {
            tracing::debug!(
                target : "nwyd::body_state", event = "item_move_send_result", ? result,
                "Item move transport result (not a server acknowledgement)"
            );
        }

        result?;

        self.last_limited_opcode = opcode;

        Ok(true)
    }

    pub fn report_desync(
        &mut self,
        position: (i32, i32),
        speed: i32,
        now_ms: u32,
    ) -> Result<bool, String> {
        let _ = (position, speed, now_ms);

        Ok(false)
    }

    pub fn select_character(
        &mut self,
        slot: usize,
        body: &mut [u8],
        now_ms: u32,
    ) -> Result<(), String> {
        let slot = AccountSlot::new(slot).ok_or_else(|| "invalid account slot".to_owned())?;

        self.select_account_slot(slot, body, now_ms)
    }

    pub fn select_account_slot(
        &mut self,
        slot: AccountSlot,
        body: &mut [u8],
        now_ms: u32,
    ) -> Result<(), String> {
        self.send_selection(body, "select character", now_ms)?;

        self.slot = Some(slot);

        self.phase = Phase::Entering;

        self.log(format!("0x213 sent — entering the world as slot {slot}"));

        Ok(())
    }

    pub fn keepalive(&mut self, idle_ms: u32, now_ms: u32) -> bool {
        if self.connection.is_none() || now_ms.wrapping_sub(self.last_send_ms) <= idle_ms {
            return false;
        }

        let sent = if idle_ms == KEEPALIVE_FIELD_MS {
            self.send_field(&mut client::keepalive(), "keepalive", now_ms)
        } else {
            self.send_selection(&mut client::keepalive(), "keepalive", now_ms)
                .map(|()| true)
        };

        match sent {
            Ok(sent) => {
                self.log_packet("→ 0x3A0 keepalive");

                sent
            }
            Err(error) => {
                self.log(error);

                false
            }
        }
    }

    pub fn quit(&mut self, now_ms: u32) {
        if self.connection.is_some() {
            match self.send(&mut client::quit(), "quit", now_ms) {
                Ok(()) => self.log("0x3AE sent — leaving"),
                Err(error) => self.log(format!("leaving without a successful quit send: {error}")),
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.drift.reset();

        if self.connection.take().is_some() {
            self.log("disconnected");
        }

        self.phase = Phase::Offline;
    }

    pub fn poll(&mut self, clock: &mut Clock) -> Vec<Transition> {
        self.drain_events()
            .into_iter()
            .filter_map(|event| self.receive_event(clock, event))
            .collect()
    }

    pub(crate) fn drain_events(&mut self) -> Vec<Event> {
        self.connection
            .as_mut()
            .map_or_else(Vec::new, |connection| connection.drain_events())
    }

    pub(crate) fn receive_event(&mut self, clock: &mut Clock, event: Event) -> Option<Transition> {
        match event {
            Event::Message { message, tick } => {
                if matches!(
                    message.as_ref(),
                    ServerMessage::LoginAccepted(_)
                        | ServerMessage::AccountBlock { .. }
                        | ServerMessage::EnterWorld(_)
                        | ServerMessage::LeaveWorld { .. }
                        | ServerMessage::ItemMoved { .. }
                ) {
                    tracing::debug!(
                        target : "nwyd::body_state", event = "body_packet_received",
                        connection = ? self.connection.as_ref().map(Connection::id),
                        opcode = message.opcode(), wire_tick = tick, slot = ? self.slot,
                        phase = ? self.phase,
                        "Body lifecycle packet before ordered dispatch"
                    );
                }

                if let Some(connection) = self.connection.as_mut() {
                    connection.apply_message_state(&message);
                }

                self.adopt_clock(clock, &message, WireTime::from_millis(tick));

                self.receive(*message)
            }
            Event::Decode(error) => {
                self.log(format!("receive framing rejected: {error}"));

                None
            }
            Event::Closed(reason) => {
                self.log_disconnect_context(clock, "closed");

                let reason = reason.unwrap_or_else(|| "the server closed the connection".into());

                self.log(format!("connection lost — {reason}"));

                self.phase = Phase::Offline;

                Some(Transition::Disconnected {
                    reason,
                    socket_closed: true,
                })
            }
        }
    }

    fn log_disconnect_context(&self, clock: &Clock, event: &str) {
        let connection = self.connection.as_ref().map(Connection::id);

        tracing::info!(
            ? connection, event, phase = ? self.phase, last_successful_send_tick = self
            .last_send_ms, wire_tick = clock.wire_tick(), clock_offset_ms = clock
            .offset_ms(), clock_sync = ? self.sync, "session at connection failure"
        );
    }

    fn adopt_clock(&mut self, clock: &mut Clock, message: &ServerMessage, tick: WireTime) {
        if let Some(desync) = self.sync.compare(clock, tick) {
            self.log_packet(format!(
                "clock drifted more than {} ms — {desync:?}, nudged {} ms",
                TimeSync::TOLERANCE_MS,
                TimeSync::STEP_MS,
            ));
        }

        match message.opcode() {
            server::LOGIN_ACCEPTED | server::ENTER_WORLD => {
                clock.synchronize(tick);

                self.log_packet(format!(
                    "clock synced to the server's tick {}",
                    tick.as_millis()
                ));
            }
            _ => {}
        }
    }

    pub fn loaded_zone(&mut self, now_ms: GameTime) {
        self.sync.loaded_zone(now_ms);
    }

    pub fn advance_field_clock(&mut self, now: GameTime) {
        self.sync.advance_field(now);
    }

    pub fn dynamic_key(&mut self, update: formats::net::dynamic_key::Update, tier: i32) {
        self.evolution_tier = tier;

        if let Some(connection) = self.connection.as_mut() {
            connection.set_dynamic_key(update.resolve(tier));

            self.log_packet("dynamic-key channel updated");
        }
    }

    pub fn remember_evolution_tier(&mut self, tier: i32) {
        self.evolution_tier = tier;
    }

    pub fn entered_server_select(&mut self) {
        self.evolution_tier = 0;
    }

    pub fn evolution_tier(&self) -> i32 {
        self.evolution_tier
    }

    pub fn left_world(&mut self) {
        self.drift.reset();

        self.phase = Phase::CharacterSelect;

        self.slot = None;

        self.sync.left_world();
    }

    pub fn replace_block(&mut self, block: Box<account::Block>) {
        tracing::debug!(
            target : "nwyd::body_state", event = "account_block_applied", connection = ?
            self.connection.as_ref().map(Connection::id), slot = ? self.slot, phase = ?
            self.phase, previous_bodies = ? self.block.as_ref().map(| b | b.slots
            .each_ref().map(| s | s.equipment.body().index)), incoming_bodies = ? block
            .slots.each_ref().map(| s | s.equipment.body().index),
            "Account body snapshot replaced"
        );

        self.block = Some(block);
    }

    fn receive(&mut self, message: ServerMessage) -> Option<Transition> {
        if tracing::enabled!(tracing::Level::INFO) {
            match &message {
                ServerMessage::ServerReturnNotice
                | ServerMessage::LoginDenied { .. }
                | ServerMessage::LoginAccepted(_)
                | ServerMessage::EnterWorld(_)
                | ServerMessage::AccountBlock { .. }
                | ServerMessage::Quit
                | ServerMessage::Redirect(_)
                | ServerMessage::Malformed { .. } => self.log(format!("← {}", message.describe())),
                _ if self.verbosity == Verbosity::Packets => {
                    self.log_packet(format!("← {}", message.describe()));
                }
                _ => {}
            }

            if let ServerMessage::Other { opcode, payload } = &message
                && self.unhandled.insert(*opcode)
            {
                self.log(format!(
                    "{opcode:#05x} is not built — {} payload byte(s), dropped (first of this session)",
                    payload.len(),
                ));
            }
        }

        match message {
            ServerMessage::QuestFlags(flags) => Some(Transition::QuestFlags(flags)),
            ServerMessage::ClientConfig(update) => Some(Transition::ClientConfig(update)),
            ServerMessage::ServerReturnNotice => Some(Transition::ServerReturnNotice),
            ServerMessage::LoginDenied { opcode } => Some(Transition::LoginDenied { opcode }),
            ServerMessage::CharacterNotice { opcode } => {
                Some(Transition::CharacterNotice { opcode })
            }
            ServerMessage::CharacterDeleteHold(hold) => Some(Transition::CharacterDeleteHold(hold)),
            ServerMessage::SelectionMetadata(value) => Some(Transition::SelectionMetadata(value)),
            ServerMessage::DynamicKey { update, .. } => Some(Transition::DynamicKey(update)),
            ServerMessage::LoginAccepted(accept) => {
                tracing::debug!(
                    target : "nwyd::body_state", event = "login_10a", connection = ? self
                    .connection.as_ref().map(Connection::id), phase = ? self.phase,
                    previous_bodies = ? self.block.as_ref().map(| b | b.slots.each_ref()
                    .map(| s | s.equipment.body().index)), wire_bodies = ? accept.block
                    .slots.each_ref().map(| s | s.equipment.body().index),
                    "Server login account bodies"
                );

                let name = accept.account();

                if !name.is_empty() {
                    self.account = name;
                }

                let redirecting = self.phase == Phase::Redirecting;

                if !redirecting {
                    if self.phase == Phase::Authenticating {
                        self.analytics.login(&self.guild_account);
                    }

                    self.phase = Phase::CharacterSelect;
                }

                self.warehouse = Some(Box::new(crate::scene::enter::Warehouse::of(&accept)));

                let block = Box::new(accept.block);

                self.block = Some(block.clone());

                (!redirecting).then(|| Transition::CharacterSelect {
                    block,
                    account: self.account.clone(),
                })
            }
            ServerMessage::AccountBlock { block, opcode } => {
                tracing::debug!(
                    target : "nwyd::body_state", event = "account_refresh_received",
                    opcode, slot = ? self.slot, wire_bodies = ? block.slots.each_ref()
                    .map(| s | s.equipment.body().index),
                    "Server account refresh before scene application"
                );

                Some(Transition::CharacterListChanged { block })
            }
            ServerMessage::EnterWorld(world) => {
                self.phase = Phase::InGame;

                Some(Transition::EnterWorld(world))
            }
            ServerMessage::LeaveWorld { client_id } => Some(Transition::LeaveWorld { client_id }),
            ServerMessage::System(system) => Some(Transition::Message(Box::new(system))),
            ServerMessage::Chat(line) => Some(Transition::Chat(line)),
            ServerMessage::Spawn { opcode, entity } => Some(Transition::Spawn { opcode, entity }),
            ServerMessage::ShopStock(stock) => Some(Transition::ShopStock(stock)),
            ServerMessage::ShopContents(contents) => Some(Transition::ShopContents(contents)),
            ServerMessage::ShopSlotSold(sold) => Some(Transition::ShopSlotSold(sold)),
            ServerMessage::ItemMoved { dst, src } => Some(Transition::ItemMoved { dst, src }),
            ServerMessage::Sold(sale) => Some(Transition::Sold(sale)),
            ServerMessage::ShopClosed => Some(Transition::ShopClosed),
            ServerMessage::CraftingClosed => Some(Transition::CraftingClosed),
            ServerMessage::AnimationProbe(request) => Some(Transition::AnimationProbe(request)),
            ServerMessage::Question(question) => Some(Transition::Question(question)),
            ServerMessage::LongMessage(message) => Some(Transition::LongMessage(message)),
            ServerMessage::ReelResult(result) => Some(Transition::ReelResult(result)),
            ServerMessage::CloseCargo => Some(Transition::CloseCargo),
            ServerMessage::QuickBar { bar } => Some(Transition::QuickBar { bar }),
            ServerMessage::Affects { entity, affects } => {
                Some(Transition::Affects { entity, affects })
            }
            ServerMessage::Action {
                opcode,
                entity,
                action,
            } => Some(Transition::Action {
                opcode,
                entity,
                action,
            }),
            ServerMessage::Remove { entity } => Some(Transition::Remove { entity }),
            ServerMessage::GroundItem(drop) => Some(Transition::GroundItem(drop)),
            ServerMessage::GateCommand(command) => Some(Transition::GateCommand(command)),
            ServerMessage::Vanish { entity, vanish } => Some(Transition::Vanish { entity, vanish }),
            ServerMessage::Attack { opcode, attack } => Some(Transition::Attack { opcode, attack }),
            ServerMessage::Kill(death) => Some(Transition::Kill(death)),
            ServerMessage::Quit => Some(Transition::Quit),
            ServerMessage::Redirect(redirect) => Some(Transition::Redirect(redirect)),
            ServerMessage::Malformed { opcode, reason } => {
                self.log(format!("{opcode:#05x} could not be read — {reason}"));

                None
            }
            ServerMessage::Vitals { entity, vitals } => Some(Transition::Vitals { entity, vitals }),
            ServerMessage::HpDelta { entity, delta } => Some(Transition::HpDelta { entity, delta }),
            ServerMessage::Firework { entity, glyph } => {
                Some(Transition::Firework { entity, glyph })
            }
            ServerMessage::Emote { entity, emote } => Some(Transition::Emote { entity, emote }),
            ServerMessage::ActorStatus { entity, score } => {
                Some(Transition::ActorStatus { entity, score })
            }
            ServerMessage::PlayerProgression { entity, score } => {
                Some(Transition::PlayerProgression { entity, score })
            }
            ServerMessage::EquipChanged { entity, change } => {
                Some(Transition::EquipChanged { entity, change })
            }
            ServerMessage::Gold { entity, gold } => Some(Transition::Gold { entity, gold }),
            ServerMessage::NameMark { entity, mode } => Some(Transition::NameMark { entity, mode }),
            ServerMessage::GlowBurst { entity, mode } => {
                Some(Transition::GlowBurst { entity, mode })
            }
            ServerMessage::GuildInvite { entity, invite } => {
                Some(Transition::GuildInvite { entity, invite })
            }
            ServerMessage::GuildApplication => Some(Transition::GuildApplication),
            ServerMessage::Challenge { entity, challenge } => {
                Some(Transition::Challenge { entity, challenge })
            }
            ServerMessage::Inventory { entity, bag } => Some(Transition::Inventory { entity, bag }),
            ServerMessage::Collection { entity, progress } => {
                Some(Transition::Collection { entity, progress })
            }
            ServerMessage::WarehouseGold(moved) => Some(Transition::WarehouseGold(moved)),
            ServerMessage::Weather(forecast) => Some(Transition::Weather(forecast)),
            ServerMessage::EventCountdown(countdown) => Some(Transition::EventCountdown(countdown)),
            ServerMessage::EventRemaining(remaining) => Some(Transition::EventRemaining(remaining)),
            ServerMessage::EventPacked(packed) => Some(Transition::EventPacked(packed)),
            ServerMessage::EventTarget(target) => Some(Transition::EventTarget(target)),
            ServerMessage::EventScore(score) => Some(Transition::EventScore(score)),
            ServerMessage::SummonRequest(request) => Some(Transition::SummonRequest(request)),
            ServerMessage::SummonCancel => Some(Transition::SummonCancel),
            ServerMessage::PlateLevel(change) => Some(Transition::PlateLevel(change)),
            ServerMessage::PlateGuild(line) => Some(Transition::PlateGuild(line)),
            ServerMessage::Guild(m) => Some(Transition::Guild(m)),
            ServerMessage::SiegeState(state) => Some(Transition::SiegeState(state)),
            ServerMessage::WarFlag(flag) => Some(Transition::WarFlag(flag)),
            ServerMessage::Kickout(k) => {
                tracing::info!(
                    connection = ? self.connection.as_ref().map(Connection::id), phase =
                    ? self.phase, code = k.code(), "server session teardown packet"
                );

                Some(Transition::Kickout(k))
            }
            ServerMessage::FieldCarpet(c) => Some(Transition::FieldCarpet(c)),
            ServerMessage::FieldSound(sound) => Some(Transition::FieldSound(sound)),
            ServerMessage::FieldLine(line) => Some(Transition::FieldLine(line)),
            ServerMessage::KeyedRecord(record) => Some(Transition::KeyedRecord(record)),
            ServerMessage::ArenaRound(round) => Some(Transition::ArenaRound(round)),
            ServerMessage::ArenaKill(kill) => Some(Transition::ArenaKill(kill)),
            ServerMessage::ArenaList(list) => Some(Transition::ArenaList(list)),
            ServerMessage::Consumed { .. } => None,
            ServerMessage::ItemPrices(update) => Some(Transition::ItemPrices(update)),
            ServerMessage::ItemArrived(arrival) => Some(Transition::ItemArrived(arrival)),
            ServerMessage::ItemCleared(clear) => Some(Transition::ItemCleared(clear)),
            ServerMessage::PartyMember { opcode, member } => {
                Some(Transition::PartyMember { opcode, member })
            }
            ServerMessage::PartyLeft(leave) => Some(Transition::PartyLeft(leave)),
            ServerMessage::PartyMarks(marks) => Some(Transition::PartyMarks(marks)),
            ServerMessage::ShopSellList(list) => Some(Transition::ShopSellList(list)),
            ServerMessage::NpcLine { entity, line } => Some(Transition::NpcLine { entity, line }),
            ServerMessage::TradeOffer { entity, offer } => {
                Some(Transition::TradeOffer { entity, offer })
            }
            ServerMessage::TradeClosed { entity } => Some(Transition::TradeClosed { entity }),
            ServerMessage::TradeToggle { entity } => Some(Transition::TradeToggle { entity }),
            ServerMessage::SlotPatch { entity, patch } => {
                Some(Transition::SlotPatch { entity, patch })
            }
            ServerMessage::Lock { opcode } => Some(Transition::AccountLock { opcode }),
            ServerMessage::CharacterSlotEffect { slot } => {
                Some(Transition::CharacterSlotEffect { slot })
            }
            ServerMessage::CharacterNameResult(reply) => {
                Some(Transition::CharacterNameResult(reply))
            }
            ServerMessage::CharacterNameNotice => Some(Transition::CharacterNameNotice),
            ServerMessage::Other { .. } => None,
        }
    }
}

fn split_port(address: &str) -> (&str, u16) {
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => match port.parse() {
            Ok(port) => (host, port),
            Err(_) => (address, transport::PORT),
        },
        _ => (address, transport::PORT),
    }
}

pub use transport::keepalive::{
    FIELD_MS as KEEPALIVE_FIELD_MS, SELECT_CHAR_MS as KEEPALIVE_SELECT_MS,
};

pub const HANDLED: [u16; 109] = [
    0x1C2,
    0x1D6,
    0x119,
    0x11A,
    0x11B,
    0x11E,
    0x120,
    0x11C,
    0x11D,
    0x194,
    0xBBF,
    0xFBC,
    0x13BD,
    0x7BE,
    0xFAA,
    0x7A9,
    server::VITALS,
    server::HP_DELTA,
    server::EMOTE,
    server::SLOT_PATCH,
    server::ACTOR_STATUS,
    server::PLAYER_PROGRESSION,
    server::EQUIP_CHANGED,
    server::GOLD,
    server::AFFECTS,
    server::MOVE,
    server::MOVE_RUN,
    server::ACT,
    server::REMOVE,
    server::GROUND_ITEM,
    server::VANISH,
    server::ATTACK,
    server::ATTACK_AREA,
    server::ATTACK_MANY,
    server::KILL,
    server::LOGIN_ACCEPTED,
    server::ACCOUNT_AFTER_DELETE,
    server::ACCOUNT_AFTER_CREATE,
    server::LOCK,
    server::LOCK_RESET,
    server::ENTER_WORLD,
    server::LEAVE_WORLD,
    server::SPAWN,
    server::SPAWN_SHOP,
    server::SYSTEM_MESSAGE,
    server::SYSTEM_MESSAGE_ARG,
    server::QUIT,
    server::REDIRECT,
    server::CHAT_SAY,
    server::CHAT_CHANNEL,
    server::CHAT_SHOUT,
    server::CHAT_CONSOLE,
    server::SHOP_STOCK,
    server::SHOP_CONTENTS,
    server::SHOP_SLOT_SOLD,
    server::ITEM_MOVED,
    server::SOLD,
    server::SHOP_CLOSED,
    server::CRAFTING_CLOSED,
    server::ANIMATION_PROBE,
    server::QUESTION,
    server::LONG_MESSAGE,
    server::REEL_RESULT,
    server::CLOSE_CARGO,
    server::QUICK_BAR,
    server::GATE_COMMAND,
    server::FIREWORK,
    server::NAME_MARK,
    server::GLOW_BURST,
    server::GUILD_INVITE,
    server::GUILD_APPLICATION,
    server::NPC_LINE,
    server::TRADE_OFFER,
    server::TRADE_CLOSED,
    server::TRADE_TOGGLE,
    server::CHALLENGE,
    server::INVENTORY,
    server::COLLECTION,
    server::WAREHOUSE_SET,
    server::WAREHOUSE_DEPOSIT,
    server::WAREHOUSE_WITHDRAW,
    server::WEATHER,
    server::ITEM_ARRIVED,
    server::ITEM_CLEARED,
    server::PARTY_JOINED,
    server::PARTY_LEFT,
    server::PARTY_INVITED,
    server::PARTY_MARKED,
    server::SHOP_SELL_LIST,
    server::EVENT_COUNTDOWN,
    server::EVENT_REMAINING,
    server::EVENT_PACKED,
    server::EVENT_TARGET,
    server::EVENT_SCORE,
    server::SUMMON_REQUEST,
    server::SUMMON_CANCEL,
    server::PLATE_LEVEL,
    server::PLATE_GUILD,
    server::GUILD,
    server::SIEGE_STATE,
    server::WAR_FLAG,
    server::KICKOUT,
    server::FIELD_CARPET,
    server::FIELD_SOUND,
    server::FIELD_LINE,
    server::KEYED_RECORD,
    server::ARENA_ROUND,
    server::ARENA_KILL,
    server::ARENA_LIST,
];

fn field_send_allowed(previous: u16, next: u16, last_send_ms: u32, now_ms: u32) -> bool {
    let deadline = last_send_ms.wrapping_add(1000);

    if deadline < now_ms {
        return true;
    }

    if previous == 0x36C && next == 0x36C {
        return false;
    }

    deadline <= now_ms
        || !matches!(previous, 0x367 | 0x39D | 0x39E)
        || !matches!(next, 0x367 | 0x39D | 0x39E)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "net/profile_tests.rs"]
mod profile_tests;

#[cfg(test)]
mod tests {

    use super::*;
    use std::time::Duration;

    #[test]
    fn movement_and_attack_throttles_keep_the_native_deadline_boundaries() {
        for (last, now, movement, attack) in [
            (25003, 25002, false, false),
            (25003, 26002, false, false),
            (25003, 26003, false, true),
            (25003, 26004, true, true),
            (0xFFFF_FFF0, 0xFFFF_FFF0, true, true),
            (0xFFFF_FFF0, 983, false, false),
            (0xFFFF_FFF0, 984, false, true),
            (0xFFFF_FFF0, 985, true, true),
        ] {
            assert_eq!(
                field_send_allowed(0x36C, 0x36C, last, now),
                movement,
                "movement: last={last}, now={now}",
            );

            for previous in [0x367, 0x39D, 0x39E] {
                for next in [0x367, 0x39D, 0x39E] {
                    assert_eq!(
                        field_send_allowed(previous, next, last, now),
                        attack,
                        "attack {previous:#x}->{next:#x}: last={last}, now={now}",
                    );
                }
            }

            for (previous, next) in [
                (0x36C, 0x39D),
                (0x39D, 0x36C),
                (0x39D, 0x3A0),
                (0x3A0, 0x36C),
                (0x333, 0x333),
            ] {
                assert!(field_send_allowed(previous, next, last, now));
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn a_forward_watermark_cannot_send_a_second_limited_packet_or_consume_its_key() {
        use std::io::Read;
        use std::net::TcpListener;

        fn request(opcode: u16) -> Vec<u8> {
            if opcode == 0x36C {
                let mut frame = vec![0; 52];
                protocol::header::set_opcode(&mut frame, opcode);
                frame
            } else {
                formats::net::attack::Attack::default().to_bytes(opcode)
            }
        }

        let pairs = std::iter::once((0x36C, 0x36C)).chain(
            [0x367, 0x39D, 0x39E]
                .into_iter()
                .flat_map(|first| [0x367, 0x39D, 0x39E].map(|second| (first, second))),
        );

        for (first, second) in pairs {
            let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
            let mut session = Session::default().with_fingerprint(Some(Fingerprint::ZERO));
            session
                .login(
                    &listener.local_addr().unwrap().to_string(),
                    "Player",
                    "testpass",
                    123,
                    5000,
                )
                .unwrap();

            let (mut peer, _) = listener.accept().unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();

            let mut login = accepted();
            if let ServerMessage::LoginAccepted(accept) = &mut login {
                accept.key = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
            }
            session
                .connection
                .as_mut()
                .unwrap()
                .apply_message_state(&login);

            assert_eq!(session.last_send_ms, 5000);
            assert!(
                session
                    .send_field(&mut request(first), "first", 25006)
                    .unwrap()
            );
            assert_eq!(session.last_send_ms, 25007);

            let mut suppressed = request(second);
            let original = suppressed.clone();
            assert!(
                !session
                    .send_field(&mut suppressed, "second", 25006)
                    .unwrap()
            );
            assert_eq!(suppressed, original);
            assert_eq!(session.last_send_ms, 25007);
            assert_eq!(session.last_limited_opcode, first);

            assert!(
                session
                    .send_field(&mut client::keepalive(), "marker", 25006)
                    .unwrap()
            );
            session.disconnect();

            let mut wire = Vec::new();
            peer.read_to_end(&mut wire).unwrap();
            assert_eq!(&wire[..4], &[0x11, 0xF3, 0x11, 0x1F]);

            let mut decoder = protocol::codec::Decoder::new();
            decoder.feed(&wire[4..]);
            let (packets, error) = decoder.drain();
            assert!(error.is_none(), "{error:?}");
            assert_eq!(packets.len(), 3);
            assert!(packets.iter().all(|packet| packet.checksum_ok));
            assert_eq!(packets[0].header.opcode, 0x20D);
            assert_eq!(packets[1].header.opcode, first);
            assert_eq!(packets[1].header.tick, 25007);
            assert_eq!(packets[1].header.key, 0xFE);
            assert_eq!(packets[2].header.opcode, 0x3A0);
            assert_eq!(packets[2].header.key, 0xFD);
        }
    }

    fn accepted() -> ServerMessage {
        let mut account = [0; account::NAME_LEN];

        account[..7].copy_from_slice(b"DISPLAY");

        ServerMessage::LoginAccepted(Box::new(account::Accept {
            key: [0; account::KEY_LEN],
            pad: [0; 4],
            block: account::Block::default(),
            cargo: [account::Item::default(); account::CARGO_SLOTS],
            extra: 0,
            account,
            tail: Vec::new(),
        }))
    }

    #[test]
    fn analytics_starts_on_accepted_login_and_preserves_the_login_identity() {
        let (analytics, events) = crate::analytics::Analytics::recording();

        let mut session = Session::default().with_analytics(analytics);

        session.guild_account = "SYNTHETIC_LOGIN".into();

        session.phase = Phase::Authenticating;

        session.receive(ServerMessage::LoginDenied { opcode: 0x11c });

        assert!(events.try_recv().is_err());

        assert!(matches!(
            session.receive(accepted()),
            Some(Transition::CharacterSelect { .. })
        ));

        let started = events.recv_timeout(Duration::from_secs(5)).unwrap();

        let device = events.recv_timeout(Duration::from_secs(5)).unwrap();

        assert_eq!(session.account, "DISPLAY");

        assert_eq!(started["eventList"][0]["eventName"], "gameStarted");

        assert_eq!(started["eventList"][0]["userID"], "SYNTHETIC_LOGIN");

        assert_eq!(device["eventList"][0]["eventName"], "clientDevice");

        session.receive(accepted());

        session.phase = Phase::Redirecting;

        assert!(session.receive(accepted()).is_none());

        session.analytics.ended();

        assert_eq!(
            events.recv_timeout(Duration::from_secs(5)).unwrap()["eventList"][0]["eventName"],
            "gameEnded"
        );
    }

    #[test]
    fn a_disabled_analytics_session_accepts_login_without_a_delivery_worker() {
        let mut session = Session::default();

        session.guild_account = "SYNTHETIC_LOGIN".into();

        session.phase = Phase::Authenticating;

        assert!(matches!(
            session.receive(accepted()),
            Some(Transition::CharacterSelect { .. })
        ));

        assert_eq!(session.phase, Phase::CharacterSelect);

        session.analytics.ended();
    }
}
