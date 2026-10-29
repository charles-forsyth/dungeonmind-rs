//! Watch mode core (engine-agnostic, testable without a window).
//!
//! * `turn_interval` / `turns_due`: speed ladder -> seconds per hero turn.
//! * `Trainer`: full-speed Q-learning in a background thread. It owns its own
//!   `QAgent` and periodically publishes an immutable `Arc<QAgent>` snapshot,
//!   so the showcase never blocks training (no lock is held while learning).
//! * `Showcase`: plays one episode with a *read-only* snapshot of the learned
//!   policy, one decision at a time (think -> act) so the GUI can display the
//!   intent, then animate the result.
use crate::qlearn::*;
use crate::rng::Rng;
use crate::sim::*;
use crate::sim::dir_of;
use crate::train::{run_episode, Config, EpStats};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Multiplier used for the "Max" speed in watch mode (turns/s = 3 * this).
pub const MAX_MULT: f32 = 64.0;

/// Seconds per hero turn for a speed multiplier.
/// 0.25x = 2.0 s, 0.5x = 1.0 s, 1x = 1/3 s (~3 turns/s), then linear (4x = 12/s).
pub fn turn_interval(mult: f32) -> f32 {
    if mult <= 0.0 {
        f32::INFINITY
    } else if mult <= 0.25 {
        2.0
    } else if mult <= 0.5 {
        1.0
    } else {
        1.0 / (3.0 * mult)
    }
}

/// Advance an accumulator by `dt` seconds and return how many turns are due
/// (capped at 64 per call so a long frame never stalls the UI).
pub fn turns_due(acc: &mut f32, dt: f32, mult: f32) -> u32 {
    let iv = turn_interval(mult);
    if !iv.is_finite() {
        *acc = 0.0;
        return 0;
    }
    *acc += dt;
    let mut n = 0;
    while *acc >= iv && n < 64 {
        *acc -= iv;
        n += 1;
    }
    if n == 64 {
        *acc = 0.0;
    }
    n
}

// ---------------------------------------------------------------- trainer

pub struct TrainShared {
    pub stop: AtomicBool,
    pub episodes: AtomicU32,
    pub q_size: AtomicU32,
    pub eps_bits: AtomicU32,
    pub rate_x100: AtomicU32,
    pub snap_episode: AtomicU32,
    pub snap_id: AtomicU64,
    snapshot: RwLock<Arc<QAgent>>,
}

pub struct Trainer {
    pub shared: Arc<TrainShared>,
    handle: Option<JoinHandle<QAgent>>,
    rx: Receiver<EpStats>,
}

impl Trainer {
    /// Start background training from `agent`. A snapshot is published every
    /// `publish_every` episodes (and at least once per second).
    pub fn start(agent: QAgent, cfg: Config, publish_every: u32) -> Trainer {
        let shared = Arc::new(TrainShared {
            stop: AtomicBool::new(false),
            episodes: AtomicU32::new(agent.episodes),
            q_size: AtomicU32::new(agent.q.len() as u32),
            eps_bits: AtomicU32::new(agent.eps.to_bits()),
            rate_x100: AtomicU32::new(0),
            snap_episode: AtomicU32::new(agent.episodes),
            snap_id: AtomicU64::new(0),
            snapshot: RwLock::new(Arc::new(agent.clone())),
        });
        let (tx, rx) = channel();
        let sh = shared.clone();
        let handle = std::thread::Builder::new()
            .name("dm-trainer".into())
            .spawn(move || {
                let mut agent = agent;
                let mut rng = Rng::new(0x5EED_0000 ^ agent.episodes as u64 ^ cfg.seed);
                let mut win_start = Instant::now();
                let mut win_eps = 0u32;
                let mut last_pub = Instant::now();
                let mut since_pub = 0u32;
                while !sh.stop.load(Ordering::Relaxed) {
                    let st = run_episode(&mut agent, &cfg, &mut rng);
                    agent.end_episode();
                    sh.episodes.store(agent.episodes, Ordering::Relaxed);
                    sh.q_size.store(agent.q.len() as u32, Ordering::Relaxed);
                    sh.eps_bits.store(agent.eps.to_bits(), Ordering::Relaxed);
                    if tx.send(st).is_err() {
                        break;
                    }
                    win_eps += 1;
                    since_pub += 1;
                    let we = win_start.elapsed().as_secs_f32();
                    if we >= 1.0 {
                        sh.rate_x100.store((win_eps as f32 / we * 100.0) as u32, Ordering::Relaxed);
                        win_start = Instant::now();
                        win_eps = 0;
                    }
                    if since_pub >= publish_every.max(1) || last_pub.elapsed() >= Duration::from_secs(1) {
                        if let Ok(mut w) = sh.snapshot.write() {
                            *w = Arc::new(agent.clone());
                        }
                        sh.snap_episode.store(agent.episodes, Ordering::Relaxed);
                        sh.snap_id.fetch_add(1, Ordering::Relaxed);
                        since_pub = 0;
                        last_pub = Instant::now();
                    }
                }
                agent
            })
            .expect("spawn trainer");
        Trainer { shared, handle: Some(handle), rx }
    }
    pub fn episodes(&self) -> u32 {
        self.shared.episodes.load(Ordering::Relaxed)
    }
    pub fn q_size(&self) -> usize {
        self.shared.q_size.load(Ordering::Relaxed) as usize
    }
    pub fn eps(&self) -> f32 {
        f32::from_bits(self.shared.eps_bits.load(Ordering::Relaxed))
    }
    pub fn eps_per_sec(&self) -> f32 {
        self.shared.rate_x100.load(Ordering::Relaxed) as f32 / 100.0
    }
    pub fn snapshot_episode(&self) -> u32 {
        self.shared.snap_episode.load(Ordering::Relaxed)
    }
    /// Latest published Q-table snapshot (cheap Arc clone).
    pub fn latest(&self) -> Arc<QAgent> {
        self.shared.snapshot.read().map(|g| g.clone()).unwrap_or_else(|e| e.into_inner().clone())
    }
    /// Episode stats finished since the last call.
    pub fn drain(&self) -> Vec<EpStats> {
        self.rx.try_iter().collect()
    }
    /// Stop the thread and take back the final learner.
    pub fn stop(mut self) -> QAgent {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.handle.take().and_then(|h| h.join().ok()).unwrap_or_default()
    }
}

impl Drop for Trainer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

// --------------------------------------------------------------- showcase

#[derive(Clone, Debug)]
pub struct Thought {
    pub action: usize,
    pub q: [f32; N_ACTIONS],
    pub legal: [bool; N_ACTIONS],
    pub why: String,
}

#[derive(Clone, Debug, Default)]
pub struct Recap {
    pub episode: u32,
    pub floor: usize,
    pub kills: u32,
    pub keys: u32,
    pub turns: u32,
    pub level: u32,
    pub gold: u32,
    pub won: bool,
    pub cause: String,
}

pub struct Showcase {
    pub snap: Arc<QAgent>,
    pub rng: Rng,
    pub eps: f32,
    pub thought: Option<Thought>,
    pub kills: u32,
    pub keys: u32,
    pub turns: u32,
    pub reward: f32,
    pub last_hurt: String,
    pub done: bool,
    pub episode_no: u32,
}

fn near_dir(a: usize, b: usize) -> bool {
    matches!((a as i32 - b as i32).rem_euclid(8), 0 | 1 | 7)
}

/// One-line human reason for the chosen action, derived from the state.
pub fn why_text(w: &World, o: &Obs, action: usize) -> String {
    match action {
        A_QUAFF => return "low HP: potion".into(),
        A_ATTACK => return "monster adjacent: attack".into(),
        A_PICKUP => return "item underfoot: pick up".into(),
        A_OPEN => {
            return if o.has_key { "have the key: open the door".into() } else { "something to open".into() };
        }
        A_DESCEND => return "on the stairs: descend".into(),
        _ => {}
    }
    // movement
    for (ix, iy, kind) in w.items.iter() {
        let i = w.map.idx(*ix, *iy);
        if !w.visible[i] {
            continue;
        }
        let d = dir_of(*ix as i32 - w.hero.x, *iy as i32 - w.hero.y);
        if !near_dir(action, d) {
            continue;
        }
        match kind {
            ItemKind::Key(_) => return "key seen: going for it".into(),
            ItemKind::Potion => return "potion seen: going for it".into(),
            ItemKind::Gold => return "gold seen: going for it".into(),
        }
    }
    let mon_vis = o.mon_dir != T_NONE;
    if mon_vis {
        let md = o.mon_dir as usize;
        let toward = near_dir(action, md);
        let away = near_dir(action, (md + 4) % 8);
        if o.hp <= 2 && away {
            return "low HP: backing away".into();
        }
        if toward && o.mon_dist <= 1 {
            return "monster close: engaging".into();
        }
        if away && o.mon_dist <= 1 {
            return "monster close: retreating".into();
        }
    }
    if o.has_key && o.door_dir != T_NONE && near_dir(action, o.door_dir as usize) {
        return "key in hand: to the locked door".into();
    }
    if o.quest_dir != T_NONE && action == o.quest_dir as usize {
        return "following the quest marker".into();
    }
    if o.stairs_dir != T_NONE && action == o.stairs_dir as usize {
        return "heading for the stairs".into();
    }
    "exploring".into()
}

impl Showcase {
    pub fn new(snap: Arc<QAgent>, seed: u64, episode_no: u32) -> Showcase {
        Showcase {
            snap,
            rng: Rng::new(seed ^ 0xA11CE),
            eps: 0.03,
            thought: None,
            kills: 0,
            keys: 0,
            turns: 0,
            reward: 0.0,
            last_hurt: String::new(),
            done: false,
            episode_no,
        }
    }

    /// Decide the next action (read-only on the snapshot) without executing it.
    pub fn think(&mut self, w: &World) -> &Thought {
        let obs = w.build_obs();
        let s = obs.key();
        let legal = w.legal();
        let action = self.snap.choose_with_eps(s, &legal, &mut self.rng, self.eps);
        let why = why_text(w, &obs, action);
        self.thought = Some(Thought { action, q: self.snap.qvals(s), legal, why });
        self.thought.as_ref().unwrap()
    }

    /// Execute the pending thought (thinking first if needed). Returns the
    /// world events produced by this turn and whether the episode ended.
    pub fn act(&mut self, w: &mut World) -> (Vec<Event>, bool) {
        if self.done {
            return (vec![], true);
        }
        if self.thought.is_none() {
            self.think(w);
        }
        let a = self.thought.take().map(|t| t.action).unwrap_or(A_PICKUP);
        let (mut r, done) = w.step(a);
        if w.dead {
            r += R_DEATH;
        }
        self.reward += r;
        self.turns += 1;
        let evs = std::mem::take(&mut w.events);
        for e in &evs {
            if let Event::Log(s) = e {
                if s.contains(" dies! +") {
                    self.kills += 1;
                }
                if s.contains(" key!") {
                    self.keys += 1;
                }
                for pat in [" hits you", " fires an arrow", " hurls eldritch", " breathes", " casts drain"] {
                    if let Some(p) = s.find(pat) {
                        self.last_hurt = s[..p].to_string();
                    }
                }
                if s.contains("hidden trap snaps") {
                    self.last_hurt = "a hidden trap".into();
                }
            }
        }
        if done {
            self.done = true;
        }
        (evs, done)
    }

    pub fn recap(&self, w: &World) -> Recap {
        let cause = if w.won {
            "Escaped the dungeon!".to_string()
        } else if w.dead {
            if self.last_hurt.is_empty() {
                "Died (unknown cause)".to_string()
            } else {
                format!("Killed by {}", self.last_hurt)
            }
        } else {
            "Wandered too long (turn limit)".to_string()
        };
        Recap {
            episode: self.episode_no,
            floor: w.deepest_floor(),
            kills: self.kills,
            keys: self.keys,
            turns: self.turns,
            level: w.hero.level,
            gold: w.hero.gold,
            won: w.won,
            cause,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_to_interval_mapping() {
        assert_eq!(turn_interval(0.25), 2.0);
        assert_eq!(turn_interval(0.5), 1.0);
        assert!((turn_interval(1.0) - 1.0 / 3.0).abs() < 1e-6);
        assert!((turn_interval(4.0) - 1.0 / 12.0).abs() < 1e-6);
        assert!(turn_interval(0.0).is_infinite());
        // monotonic: faster speed -> shorter interval
        let ladder = [0.25, 0.5, 1.0, 4.0, 16.0, MAX_MULT];
        for w in ladder.windows(2) {
            assert!(turn_interval(w[1]) < turn_interval(w[0]));
        }
    }

    #[test]
    fn turns_due_accumulates() {
        let mut acc = 0.0;
        // 0.5x: one turn per second
        let mut n = 0;
        for _ in 0..125 {
            n += turns_due(&mut acc, 1.0 / 60.0, 0.5);
        }
        assert_eq!(n, 2, "~2 s of frames at 0.5x = 2 turns");
        // 1x: ~3 turns/s
        let mut acc = 0.0;
        let mut n = 0;
        for _ in 0..60 {
            n += turns_due(&mut acc, 1.0 / 60.0, 1.0);
        }
        assert!((2..=3).contains(&n), "got {}", n);
        // paused
        assert_eq!(turns_due(&mut acc, 1.0, 0.0), 0);
        // giant frame is capped
        let mut acc = 0.0;
        assert!(turns_due(&mut acc, 100.0, 16.0) <= 64);
    }

    #[test]
    fn showcase_thinks_without_mutating_snapshot() {
        let mut agent = QAgent::new();
        agent.eps = 0.5;
        let snap = Arc::new(agent);
        let before = snap.q.len();
        let mut w = World::new(11);
        let mut sc = Showcase::new(snap.clone(), 1, 1);
        for _ in 0..20 {
            sc.think(&w);
            let (_e, done) = sc.act(&mut w);
            if done {
                break;
            }
        }
        assert_eq!(snap.q.len(), before, "showcase must not learn");
        assert_eq!(Arc::strong_count(&snap) >= 2, true);
    }

    #[test]
    fn training_continues_while_showcase_episode_is_running() {
        let agent = QAgent::new();
        let cfg = Config { seed_mode_fixed: false, seed: 5, max_steps: 300 };
        let trainer = Trainer::start(agent, cfg, 5);
        let start_eps = trainer.episodes();
        let snap0 = trainer.latest();
        let mut world = World::new(42);
        let mut sc = Showcase::new(snap0.clone(), 9, 1);
        // The showcase is mid-episode (a handful of turns, hero alive) ...
        for _ in 0..4 {
            sc.think(&world);
            sc.act(&mut world);
        }
        assert!(!sc.done && !world.dead && !world.won);
        // ... while the trainer finishes several full episodes in the background.
        let t0 = Instant::now();
        while trainer.episodes() < start_eps + 8 && t0.elapsed() < Duration::from_secs(60) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(trainer.episodes() >= start_eps + 8, "trainer did not advance");
        // showcase episode is still running and its snapshot is untouched
        assert!(!sc.done);
        assert_eq!(snap0.episodes, start_eps);
        // trainer published a newer snapshot for the NEXT showcase episode
        let t0 = Instant::now();
        while trainer.latest().episodes <= start_eps && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(trainer.latest().episodes > start_eps);
        assert!(trainer.latest().q.len() >= snap0.q.len());
        let learner = trainer.stop();
        assert!(learner.episodes >= start_eps + 8);
    }

    #[test]
    fn why_text_basics() {
        let w = World::new(3);
        let o = w.build_obs();
        assert_eq!(why_text(&w, &o, A_QUAFF), "low HP: potion");
        assert_eq!(why_text(&w, &o, A_ATTACK), "monster adjacent: attack");
        assert_eq!(why_text(&w, &o, A_DESCEND), "on the stairs: descend");
    }
}
