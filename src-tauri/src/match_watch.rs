//! When a League match starts and ends, and whether League lighting owns the keyboard.

/// How often to look: for a match while idle, at it while one runs. After GONE_MS without
/// match data (left, crashed) the match counts as over.
pub const SEARCH_MS: u64 = 1000;
pub const POLL_MS: u64 = 300;
pub const GONE_MS: u64 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    /// No endpoint, an error reply, or a match you're not playing in.
    Nothing,
    Playing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Started,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Live { last_seen: u64, aside: bool },
    /// Ended by GameEnd: the end screen still answers, so wait for it to go.
    Over,
}

pub struct Watch {
    state: State,
}

impl Watch {
    pub fn new() -> Self {
        Watch { state: State::Idle }
    }

    pub fn poll(&mut self, p: Poll, now_ms: u64) -> Option<Change> {
        match (self.state, p) {
            (State::Idle, Poll::Playing) => {
                self.state = State::Live { last_seen: now_ms, aside: false };
                Some(Change::Started)
            }
            (State::Live { aside, .. }, Poll::Playing) => {
                self.state = State::Live { last_seen: now_ms, aside };
                None
            }
            (State::Live { last_seen, .. }, Poll::Nothing) if now_ms.saturating_sub(last_seen) >= GONE_MS => {
                self.state = State::Idle;
                Some(Change::Ended)
            }
            (State::Over, Poll::Nothing) => {
                self.state = State::Idle;
                None
            }
            _ => None,
        }
    }

    /// The match reported its result.
    pub fn game_end(&mut self) -> Option<Change> {
        matches!(self.state, State::Live { .. }).then(|| {
            self.state = State::Over;
            Change::Ended
        })
    }

    /// The owner started something else: leave the keyboard alone until the next match.
    pub fn step_aside(&mut self) {
        if let State::Live { last_seen, .. } = self.state {
            self.state = State::Live { last_seen, aside: true };
        }
    }

    pub fn owns_keyboard(&self) -> bool {
        matches!(self.state, State::Live { aside: false, .. })
    }

    pub fn live(&self) -> bool {
        matches!(self.state, State::Live { .. })
    }
}

impl Default for Watch {
    fn default() -> Self {
        Watch::new()
    }
}

/// The watcher: looks for a match, and while League lighting owns the keyboard streams its
/// frames through the shared keyboard (cable or dongle), like the animations.
#[cfg(windows)]
pub mod run {
    use super::*;
    use crate::device::Keyboard;
    use crate::gamelight::{activate, render, Active, Event as Moment, Setup};
    use crate::league::{self, Parsed, Snapshot, Tracker};
    use crate::protocol::Rgb;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    pub struct Handle {
        pub setup: Arc<Mutex<Setup>>,
        pub stop: Arc<AtomicBool>,
        /// Set when the owner starts something else: League steps aside for this match.
        pub step_aside: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Handle {
        pub fn stop(mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    pub enum Event {
        /// A match started. The answer: take the keyboard (true) or stay aside (false).
        Started,
        /// The match is over (after its result effect).
        Ended,
        Frame(Vec<(u8, Rgb)>),
        Error(String),
    }

    pub fn start(
        kb: Arc<Mutex<Option<Keyboard>>>,
        open: fn() -> crate::device::Result<Keyboard>,
        setup: Setup,
        on: impl Fn(Event) -> bool + Send + 'static,
    ) -> Handle {
        let stop = Arc::new(AtomicBool::new(false));
        let step_aside = Arc::new(AtomicBool::new(false));
        let setup = Arc::new(Mutex::new(setup));
        let (stop2, aside2, setup2) = (stop.clone(), step_aside.clone(), setup.clone());
        let thread = std::thread::spawn(move || {
            let leds = crate::audio::leds();
            let clock = Instant::now();
            let period = Duration::from_secs_f32(1.0 / crate::audio::FPS as f32);
            let mut watch = Watch::new();
            let mut tracker = Tracker::new();
            let mut active: Vec<Active> = Vec::new();
            let mut last: Option<Snapshot> = None;
            // the result effect plays to its end before the match counts as over
            let mut ending: Option<f32> = None;
            let mut next_poll = Instant::now();
            let (mut fetch_failures, mut send_failures) = (0u32, 0u32);
            let report = |n: &mut u32, e: String| {
                *n += 1;
                if *n == 1 || *n % 30 == 0 {
                    on(Event::Error(e));
                }
            };
            while !stop2.load(Ordering::Relaxed) {
                let tick = Instant::now();
                let now = clock.elapsed().as_secs_f32();
                if aside2.swap(false, Ordering::Relaxed) {
                    watch.step_aside();
                }
                if tick >= next_poll {
                    let mut found = None;
                    if watch.live() || league::running() {
                        match league::fetch().and_then(|b| b.map(|b| league::parse(&b)).transpose()) {
                            Ok(Some(Parsed::Playing(s, e, p))) => found = Some((s, e, p)),
                            Ok(_) => {}
                            Err(e) => report(&mut fetch_failures, e),
                        }
                    }
                    let poll = if found.is_some() { Poll::Playing } else { Poll::Nothing };
                    match watch.poll(poll, clock.elapsed().as_millis() as u64) {
                        Some(Change::Started) => {
                            tracker = Tracker::new();
                            active.clear();
                            ending = None;
                            if !on(Event::Started) {
                                watch.step_aside();
                            }
                        }
                        Some(Change::Ended) => {
                            ending = None;
                            last = None;
                            on(Event::Ended);
                        }
                        None => {}
                    }
                    if let Some((s, e, p)) = found {
                        fetch_failures = 0;
                        let owned = watch.owns_keyboard();
                        for ev in tracker.update(&s, &e, &p) {
                            let result = matches!(ev.event, Moment::Win | Moment::Lose);
                            let fx = activate(&setup2.lock().unwrap(), &ev, now);
                            if owned {
                                active.extend(fx.clone());
                            }
                            if result && watch.game_end().is_some() {
                                let end = if owned { fx.map_or(now, |a| a.start + a.ms_total()) } else { now };
                                ending = Some(end);
                            }
                        }
                        last = Some(s);
                    }
                    next_poll = tick + Duration::from_millis(if watch.live() { POLL_MS } else { SEARCH_MS });
                }
                if let Some(end) = ending {
                    if now >= end {
                        ending = None;
                        last = None;
                        active.clear();
                        on(Event::Ended);
                    }
                }
                if let (true, Some(s)) = (watch.owns_keyboard() || ending.is_some(), last.as_ref()) {
                    let fraction = tracker.respawn_fraction(s);
                    let groups = crate::anim::fit(render(&setup2.lock().unwrap(), s, fraction, &mut active, &leds, now));
                    match crate::device::send_frame(&kb, open, &groups) {
                        Ok(_) => send_failures = 0,
                        Err(e) => {
                            report(&mut send_failures, e);
                            std::thread::sleep(Duration::from_millis(500));
                        }
                    }
                    on(Event::Frame(groups.iter().flat_map(|(c, ks)| ks.iter().map(move |&i| (i, *c))).collect()));
                }
                if let Some(rest) = period.checked_sub(tick.elapsed()) {
                    std::thread::sleep(rest);
                }
            }
        });
        Handle { setup, stop, step_aside, thread: Some(thread) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_starts_a_match_once() {
        let mut w = Watch::new();
        assert_eq!(w.poll(Poll::Nothing, 0), None);
        assert_eq!(w.poll(Poll::Playing, 1000), Some(Change::Started));
        assert_eq!(w.poll(Poll::Playing, 1300), None);
        assert!(w.owns_keyboard());
    }

    #[test]
    fn endpoint_gone_for_three_seconds_ends_the_match() {
        let mut w = Watch::new();
        w.poll(Poll::Playing, 0);
        assert_eq!(w.poll(Poll::Nothing, 1000), None);
        assert_eq!(w.poll(Poll::Nothing, 2000), None);
        assert_eq!(w.poll(Poll::Nothing, 3000), Some(Change::Ended));
        assert!(!w.owns_keyboard());
    }

    #[test]
    fn game_end_ends_at_once() {
        let mut w = Watch::new();
        w.poll(Poll::Playing, 0);
        assert_eq!(w.game_end(), Some(Change::Ended));
        assert!(!w.owns_keyboard());
    }

    // the endpoint keeps answering on the end-of-game screen
    #[test]
    fn the_end_screen_is_not_a_new_match() {
        let mut w = Watch::new();
        w.poll(Poll::Playing, 0);
        w.game_end();
        assert_eq!(w.poll(Poll::Playing, 300), None);
        assert_eq!(w.poll(Poll::Nothing, 600), None);
        assert_eq!(w.poll(Poll::Playing, 60_000), Some(Change::Started));
    }

    #[test]
    fn stepping_aside_lasts_until_the_next_match() {
        let mut w = Watch::new();
        assert_eq!(w.poll(Poll::Playing, 0), Some(Change::Started));
        w.step_aside();
        assert!(!w.owns_keyboard());
        assert_eq!(w.poll(Poll::Playing, 300), None);
        assert!(!w.owns_keyboard());
        assert_eq!(w.poll(Poll::Nothing, 3300), Some(Change::Ended));
        assert_eq!(w.poll(Poll::Playing, 90_000), Some(Change::Started));
        assert!(w.owns_keyboard());
    }
}
