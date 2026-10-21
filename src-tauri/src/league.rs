//! League of Legends: the live match data Riot's game serves on this PC
//! (Live Client Data API, `https://127.0.0.1:2999/liveclientdata/`, only during a match).

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::WebPkiServerVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, RootCertStore, SignatureScheme};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

// ------------------------------------------------------------ the local endpoint

const ALL_GAME_DATA: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

/// Riot's root certificate for the game's local endpoints (from Riot's developer docs; the
/// copy in RiotGames/leaguedirector, identical to two independent copies).
fn riot_roots() -> RootCertStore {
    let mut roots = RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut &include_bytes!("../certs/riotgames.pem")[..]).flatten() {
        let _ = roots.add(cert);
    }
    roots
}

/// The game's certificate chains to Riot's root but isn't issued for 127.0.0.1, so the name
/// is the one thing not checked. Only ever used for the fixed local address.
#[derive(Debug)]
struct RiotOnly(Arc<WebPkiServerVerifier>);

impl ServerCertVerifier for RiotOnly {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        match self.0.verify_server_cert(end_entity, intermediates, server_name, ocsp, now) {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
            )) => Ok(ServerCertVerified::assertion()),
            other => other,
        }
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls12_signature(message, cert, dss)
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.0.verify_tls13_signature(message, cert, dss)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.supported_verify_schemes()
    }
}

fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(riot_roots()), provider.clone())
            .build()
            .expect("Riot's root certificate");
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("TLS versions")
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(RiotOnly(verifier)))
            .with_no_client_auth();
        ureq::AgentBuilder::new().tls_config(Arc::new(tls)).timeout(Duration::from_millis(500)).build()
    })
}

/// What the endpoint answers when there's no match data yet (the loading screen).
fn is_error_reply(body: &str) -> bool {
    body.trim_start().starts_with('{') && body.contains("\"errorCode\"")
}

/// The live match data, or None when there's no match: nothing listening, a 404, or an
/// error reply. Err only for a bad certificate or another HTTP error.
pub fn fetch() -> Result<Option<String>, String> {
    match agent().get(ALL_GAME_DATA).call() {
        Ok(r) => {
            let body = r.into_string().map_err(|e| e.to_string())?;
            Ok((!is_error_reply(&body)).then_some(body))
        }
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(format!("League's live data answered {code}.")),
        Err(ureq::Error::Transport(t)) => {
            // refused or timed out: the game isn't serving (no match); anything else is real
            let mut source = std::error::Error::source(&t);
            while let Some(e) = source {
                if let Some(io) = e.downcast_ref::<std::io::Error>() {
                    use std::io::ErrorKind::*;
                    if matches!(io.kind(), ConnectionRefused | TimedOut | WouldBlock | ConnectionReset) {
                        return Ok(None);
                    }
                }
                source = e.source();
            }
            Err(format!("Can't read League's live data: {t}"))
        }
    }
}

// ------------------------------------------------------------ the match data

use crate::gamelight::Event;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Team {
    Order,
    Chaos,
}

impl Team {
    fn parse(s: &str) -> Option<Team> {
        match s.to_ascii_uppercase().as_str() {
            "ORDER" => Some(Team::Order),
            "CHAOS" => Some(Team::Chaos),
            _ => None,
        }
    }
}

/// You, right now.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub team: Team,
    pub health: f32,
    pub max_health: f32,
    pub resource: f32,
    pub resource_max: f32,
    /// "MANA", "ENERGY", "NONE", ...
    pub resource_kind: String,
    pub level: u32,
    pub gold: f32,
    pub dead: bool,
    /// Seconds until you respawn.
    pub respawn: f32,
}

/// One entry of the match's event list, as it came.
#[derive(Debug, Clone)]
pub struct RawEvent {
    pub id: u64,
    pub name: String,
    pub fields: Value,
}

/// A name as compared: lower case, without a `#TAG`. Since the Riot ID change the API names
/// players inconsistently (with or without the tag, or by the old summoner name).
fn norm(name: &str) -> String {
    name.split('#').next().unwrap_or("").trim().to_lowercase()
}

/// Who's who: your names, and every player's names with their team.
#[derive(Debug, Clone, Default)]
pub struct Players {
    me: Vec<String>,
    teams: HashMap<String, Team>,
}

impl Players {
    fn is_me(&self, name: &str) -> bool {
        let n = norm(name);
        !n.is_empty() && self.me.contains(&n)
    }
    fn team_of(&self, name: &str) -> Option<Team> {
        self.teams.get(&norm(name)).copied()
    }
}

fn names(v: &Value) -> Vec<String> {
    ["riotId", "riotIdGameName", "summonerName"]
        .iter()
        .filter_map(|k| v.get(k).and_then(Value::as_str))
        .map(norm)
        .filter(|n| !n.is_empty())
        .collect()
}

pub enum Parsed {
    /// No match, or one you're not playing in (spectating, a replay).
    NotPlaying,
    Playing(Snapshot, Vec<RawEvent>, Players),
}

/// Read an `allgamedata` reply.
pub fn parse(json: &str) -> Result<Parsed, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("League's live data didn't parse: {e}"))?;
    let me = &v["activePlayer"];
    let stats = &me["championStats"];
    if !stats.is_object() {
        return Ok(Parsed::NotPlaying);
    }
    let num = |x: &Value| x.as_f64().unwrap_or(0.0) as f32;
    let mut players = Players { me: names(me), ..Default::default() };
    let mut mine: Option<&Value> = None;
    for p in v["allPlayers"].as_array().into_iter().flatten() {
        let ns = names(p);
        if let Some(t) = p["team"].as_str().and_then(Team::parse) {
            for n in &ns {
                players.teams.insert(n.clone(), t);
            }
        }
        if mine.is_none() && ns.iter().any(|n| players.me.contains(n)) {
            mine = Some(p);
        }
    }
    let health = num(&stats["currentHealth"]);
    let snap = Snapshot {
        // not found in the player list (a naming change): assume blue side, dead at 0 health
        team: mine.and_then(|p| p["team"].as_str()).and_then(Team::parse).unwrap_or(Team::Order),
        health,
        max_health: num(&stats["maxHealth"]),
        resource: num(&stats["resourceValue"]),
        resource_max: num(&stats["resourceMax"]),
        resource_kind: stats["resourceType"].as_str().unwrap_or("NONE").to_string(),
        level: me["level"].as_u64().unwrap_or(1) as u32,
        gold: num(&me["currentGold"]),
        dead: mine.map_or(health <= 0.0, |p| p["isDead"].as_bool().unwrap_or(false)),
        respawn: mine.map_or(0.0, |p| num(&p["respawnTimer"])),
    };
    let events = v["events"]["Events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            Some(RawEvent { id: e["EventID"].as_u64()?, name: e["EventName"].as_str()?.to_string(), fields: e.clone() })
        })
        .collect();
    Ok(Parsed::Playing(snap, events, players))
}

/// A moment of yours an effect can play for.
#[derive(Debug, Clone, PartialEq)]
pub struct GameEvent {
    pub event: Event,
    /// Multi-kills: 2 (double) to 5 (penta).
    pub streak: u8,
    /// Dragons: the drake type ("Fire", "Water", ...).
    pub dragon: Option<String>,
}

impl GameEvent {
    fn new(event: Event) -> Self {
        GameEvent { event, streak: 0, dragon: None }
    }
}

/// Turns the growing event list into your new events, once each.
#[derive(Debug, Default)]
pub struct Tracker {
    last_id: Option<u64>,
    level: u32,
    /// Longest respawn timer seen during this death.
    respawn_total: f32,
}

impl Tracker {
    pub fn new() -> Self {
        Tracker::default()
    }

    /// Your events since the last call. The first call only sets the baseline, so joining a
    /// match halfway doesn't replay what already happened.
    pub fn update(&mut self, snap: &Snapshot, events: &[RawEvent], players: &Players) -> Vec<GameEvent> {
        self.respawn_total = if snap.dead { self.respawn_total.max(snap.respawn) } else { 0.0 };
        let newest = events.iter().map(|e| e.id).max();
        let Some(last) = self.last_id else {
            self.last_id = Some(newest.unwrap_or(0));
            self.level = snap.level;
            return Vec::new();
        };
        let mut out: Vec<GameEvent> =
            events.iter().filter(|e| e.id > last).flat_map(|e| classify(e, snap.team, players)).collect();
        self.last_id = Some(newest.unwrap_or(last).max(last));
        if snap.level > self.level {
            out.push(GameEvent::new(Event::LevelUp));
        }
        self.level = snap.level;
        out
    }

    /// Respawn bar: 1 just after dying, 0 when back. 0 while alive.
    pub fn respawn_fraction(&self, snap: &Snapshot) -> f32 {
        if !snap.dead || self.respawn_total <= 0.0 {
            return 0.0;
        }
        (snap.respawn / self.respawn_total).clamp(0.0, 1.0)
    }
}

/// The team a structure (`Turret_T1_...`, `Barracks_T2_...`) belongs to.
fn structure_team(name: &str) -> Option<Team> {
    if name.contains("_T1_") {
        Some(Team::Order)
    } else if name.contains("_T2_") {
        Some(Team::Chaos)
    } else {
        None
    }
}

fn classify(e: &RawEvent, team: Team, players: &Players) -> Vec<GameEvent> {
    let f = &e.fields;
    let s = |k: &str| f[k].as_str().unwrap_or("");
    let ours = |name: &str| players.is_me(name) || players.team_of(name) == Some(team);
    let mut out = Vec::new();
    match e.name.as_str() {
        "ChampionKill" => {
            if players.is_me(s("KillerName")) {
                out.push(GameEvent::new(Event::Kill));
            }
            if players.is_me(s("VictimName")) {
                out.push(GameEvent::new(Event::Death));
            }
            let assisters = f["Assisters"].as_array().into_iter().flatten().filter_map(Value::as_str);
            if assisters.into_iter().any(|a| players.is_me(a)) {
                out.push(GameEvent::new(Event::Assist));
            }
        }
        "FirstBlood" if players.is_me(s("Recipient")) => out.push(GameEvent::new(Event::FirstBlood)),
        "Multikill" if players.is_me(s("KillerName")) => out.push(GameEvent {
            streak: f["KillStreak"].as_u64().unwrap_or(2).min(5) as u8,
            ..GameEvent::new(Event::Multikill)
        }),
        "Ace" if Team::parse(s("AcingTeam")) == Some(team) => out.push(GameEvent::new(Event::Ace)),
        "DragonKill" if ours(s("KillerName")) => {
            out.push(GameEvent { dragon: Some(s("DragonType").to_string()), ..GameEvent::new(Event::Dragon) })
        }
        "HeraldKill" if ours(s("KillerName")) => out.push(GameEvent::new(Event::Herald)),
        "BaronKill" if ours(s("KillerName")) => out.push(GameEvent::new(Event::Baron)),
        "TurretKilled" if structure_team(s("TurretKilled")).is_some_and(|t| t != team) => {
            out.push(GameEvent::new(Event::Turret))
        }
        "InhibKilled" if structure_team(s("InhibKilled")).is_some_and(|t| t != team) => {
            out.push(GameEvent::new(Event::Inhibitor))
        }
        "GameEnd" => match s("Result") {
            "Win" => out.push(GameEvent::new(Event::Win)),
            "Lose" => out.push(GameEvent::new(Event::Lose)),
            _ => {}
        },
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture(name: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/league").join(name);
        std::fs::read_to_string(path).unwrap()
    }

    fn playing(name: &str) -> (Snapshot, Vec<RawEvent>, Players) {
        match parse(&fixture(name)).unwrap() {
            Parsed::Playing(s, e, p) => (s, e, p),
            Parsed::NotPlaying => panic!("{name} should be a match"),
        }
    }

    fn ev(id: u64, name: &str, fields: serde_json::Value) -> RawEvent {
        RawEvent { id, name: name.into(), fields }
    }

    fn kinds(evs: &[GameEvent]) -> Vec<Event> {
        evs.iter().map(|e| e.event).collect()
    }

    #[test]
    fn in_game_data_parses_into_a_snapshot() {
        let (s, events, _) = playing("in_game.json");
        assert_eq!(s.team, Team::Order);
        assert_eq!((s.health, s.max_health), (600.0, 1200.0));
        assert_eq!((s.resource, s.resource_max, s.resource_kind.as_str()), (150.0, 300.0, "MANA"));
        assert_eq!((s.level, s.gold, s.dead), (6, 1234.5, false));
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn spectator_data_is_not_a_match() {
        assert!(matches!(parse(&fixture("spectator.json")).unwrap(), Parsed::NotPlaying));
    }

    #[test]
    fn first_poll_sets_a_baseline_without_effects() {
        let (s, e, p) = playing("in_game.json"); // already holds a kill by you
        assert!(Tracker::new().update(&s, &e, &p).is_empty());
    }

    #[test]
    fn new_events_after_the_baseline_are_yours() {
        let (s, mut e, p) = playing("in_game.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        e.push(ev(3, "ChampionKill", json!({"KillerName": "Kai", "VictimName": "Foe", "Assisters": []})));
        assert_eq!(kinds(&t.update(&s, &e, &p)), [Event::Kill]);
        assert!(t.update(&s, &e, &p).is_empty());
    }

    #[test]
    fn events_match_you_by_any_of_your_names() {
        let (s, mut e, p) = playing("riot_id.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        e.push(ev(3, "ChampionKill", json!({"KillerName": "Kai", "VictimName": "Foe", "Assisters": []})));
        e.push(ev(4, "ChampionKill", json!({"KillerName": "Foe", "VictimName": "kai#euw", "Assisters": []})));
        e.push(ev(5, "ChampionKill", json!({"KillerName": "Ally", "VictimName": "Foe", "Assisters": ["KAI#EUW"]})));
        assert_eq!(kinds(&t.update(&s, &e, &p)), [Event::Kill, Event::Death, Event::Assist]);
    }

    #[test]
    fn objectives_and_structures_count_for_your_team_only() {
        let (s, mut e, p) = playing("in_game.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        e.push(ev(3, "DragonKill", json!({"KillerName": "Ally", "DragonType": "Fire", "Stolen": "False", "Assisters": []})));
        e.push(ev(4, "DragonKill", json!({"KillerName": "Foe", "DragonType": "Water", "Stolen": "False", "Assisters": []})));
        e.push(ev(5, "TurretKilled", json!({"KillerName": "Ally", "TurretKilled": "Turret_T2_L_03_A", "Assisters": []})));
        e.push(ev(6, "TurretKilled", json!({"KillerName": "Foe", "TurretKilled": "Turret_T1_C_05_A", "Assisters": []})));
        e.push(ev(7, "BaronKill", json!({"KillerName": "Foe", "Stolen": "False", "Assisters": []})));
        let got = t.update(&s, &e, &p);
        assert_eq!(kinds(&got), [Event::Dragon, Event::Turret]);
        assert_eq!(got[0].dragon.as_deref(), Some("Fire"));
    }

    #[test]
    fn multikill_carries_its_streak() {
        let (s, mut e, p) = playing("in_game.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        e.push(ev(3, "Multikill", json!({"KillerName": "Kai", "KillStreak": 5})));
        let got = t.update(&s, &e, &p);
        assert_eq!((got[0].event, got[0].streak), (Event::Multikill, 5));
    }

    #[test]
    fn game_end_reports_win_or_lose() {
        let (s, e, p) = playing("in_game.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        let (s, e, p) = playing("game_end.json"); // adds an ally's turret and GameEnd Win
        assert_eq!(kinds(&t.update(&s, &e, &p)), [Event::Turret, Event::Win]);
    }

    #[test]
    fn level_up_and_respawn_fraction() {
        let (mut s, e, p) = playing("in_game.json");
        let mut t = Tracker::new();
        t.update(&s, &e, &p);
        s.level = 7;
        assert_eq!(kinds(&t.update(&s, &e, &p)), [Event::LevelUp]);
        let (mut d, e, p) = playing("dead.json");
        assert!(d.dead);
        t.update(&d, &e, &p);
        assert_eq!(t.respawn_fraction(&d), 1.0);
        d.respawn = 11.75;
        t.update(&d, &e, &p);
        assert_eq!(t.respawn_fraction(&d), 0.5);
    }

    #[test]
    fn riot_root_certificate_loads() {
        assert_eq!(riot_roots().len(), 1);
    }

    // what the endpoint answers during the loading screen
    #[test]
    fn an_error_reply_is_no_match() {
        assert!(is_error_reply(r#"{"errorCode":"RESOURCE_NOT_FOUND","httpStatus":404,"message":"..."}"#));
        assert!(!is_error_reply(r#"{"activePlayer":{"riotId":"Kai#EUW"}}"#));
    }

}
