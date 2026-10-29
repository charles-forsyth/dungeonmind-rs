// DungeonMind GUI: macroquad renderer + live Q-learning driver.
use crate::gui_art::*;
use dungeonmind::dungeon::*;
use dungeonmind::entities::Kind;
use dungeonmind::qlearn::{QAgent, N_ACTIONS, A_ATTACK, A_QUAFF, A_PICKUP, A_OPEN};
use dungeonmind::rng::Rng;
use dungeonmind::sim::*;
use dungeonmind::train::{self, Config, EpStats};
use dungeonmind::watch::{self, Recap, Showcase, Trainer, MAX_MULT};
use macroquad::prelude::*;


pub const PANEL_W: f32 = 340.0;
pub const HUD_H: f32 = 68.0;
pub const TILE: f32 = 22.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Speed {
    Paused,
    Q25,
    Q50,
    X1,
    X4,
    X16,
    Max,
}

impl Speed {
    pub const ALL: [Speed; 7] = [Speed::Paused, Speed::Q25, Speed::Q50, Speed::X1, Speed::X4, Speed::X16, Speed::Max];
    /// speed multiplier fed to `watch::turn_interval` (1x = ~3 turns/s).
    pub fn mult(self) -> f32 {
        match self {
            Speed::Paused => 0.0,
            Speed::Q25 => 0.25,
            Speed::Q50 => 0.5,
            Speed::X1 => 1.0,
            Speed::X4 => 4.0,
            Speed::X16 => 16.0,
            Speed::Max => MAX_MULT,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Speed::Paused => "PAUSE",
            Speed::Q25 => "0.25x",
            Speed::Q50 => "0.5x",
            Speed::X1 => "1x",
            Speed::X4 => "4x",
            Speed::X16 => "16x",
            Speed::Max => "MAX",
        }
    }
    /// fast-forward speeds skip FX generation
    pub fn is_fast(self) -> bool {
        matches!(self, Speed::X16 | Speed::Max)
    }
}

#[derive(Clone, Copy)]
pub struct Lunge {
    pub dir: Vec2,
    pub t: f32,
    pub dur: f32,
}
/// Visual state of one actor: tweened tile position, hit flash, attack lunge.
#[derive(Clone, Copy)]
pub struct Anim {
    pub pos: Vec2,
    pub flash: f32,
    pub lunge: Option<Lunge>,
}
impl Anim {
    pub fn at(x: i32, y: i32) -> Anim {
        Anim { pos: Vec2::new(x as f32, y as f32), flash: 0.0, lunge: None }
    }
    /// extra offset in tile units from an attack lunge
    pub fn lunge_off(&self) -> Vec2 {
        match self.lunge {
            Some(l) => l.dir * 0.42 * (std::f32::consts::PI * (l.t / l.dur).clamp(0.0, 1.0)).sin(),
            None => Vec2::ZERO,
        }
    }
}
pub struct Dice {
    pub x: f32,
    pub y: f32,
    pub life: f32,
    pub d20: i32,
    pub bonus: i32,
    pub ac: i32,
    pub hit: bool,
    pub hero: bool,
}

pub struct FloatText {
    pub x: f32,
    pub y: f32,
    pub vy: f32,
    pub life: f32,
    pub txt: String,
    pub col: Color,
    pub size: f32,
}
pub struct Part {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max: f32,
    pub col: Color,
    pub grav: f32,
    pub size: f32,
}

pub struct Game {
    pub world: World,
    pub agent: QAgent,
    pub cfg: Config,
    pub ep_rng: Rng,
    pub speed: Speed,
    pub step_once: bool,
    pub heatmap: u8, // 0 off, 1 visits, 2 explored
    pub log: Vec<String>,
    pub floats: Vec<FloatText>,
    pub parts: Vec<Part>,
    pub shake: f32,
    pub cam: Vec2,
    pub hist: Hist,
    pub cur_q: [f32; N_ACTIONS],
    pub cur_legal: [bool; N_ACTIONS],
    pub cur_action: usize,
    pub ep_reward: f32,
    pub ep_turns: u32,
    pub episodes_total: u32,
    pub seed_fixed: bool,
    pub cur_seed: u64,
    pub save_ctr: u32,
    pub time: f32,
    pub fps_fast: u32,
    pub sim_acc: f32,
    pub last_save: f32,
    // --- watch / camera ---
    pub zoom: f32,
    pub zoom_target: f32,
    pub follow: bool,
    pub watch: bool,
    pub thought_on: bool,
    pub trainer: Option<Trainer>,
    pub showcase: Option<Showcase>,
    pub recap: Option<Recap>,
    pub sc_pause: f32,
    pub sc_no: u32,
    pub hero_anim: Anim,
    pub mon_anim: Vec<Anim>,
    pub anim_key: (u64, usize),
    pub dice: Vec<Dice>,
    pub slowmo: f32,
    pub hint_t: f32,
    pub cur_why: String,
    pub hist_stride: u32,
    pub hist_ctr: u32,
    pub last_speed: Speed,
    pub last_click: f32,
    pub drag_on: bool,
    pub drag_moved: bool,
    pub drag_start: Vec2,
    pub drag_last: Vec2,
    pub sc_floor_seen: usize,
}

#[derive(Default)]
pub struct Hist {
    pub rew: Vec<f32>,
    pub ma: Vec<f32>,
    pub floor: Vec<f32>,
    pub eps: Vec<f32>,
    pub qsz: Vec<f32>,
    pub win: Vec<f32>,
}

impl Game {
    pub fn fresh(seed: u64, fixed: bool, agent: QAgent) -> Game {
        Game {
            world: World::new(seed),
            agent,
            cfg: Config {
                seed_mode_fixed: fixed,
                seed,
                max_steps: 2000,
            },
            ep_rng: Rng::new(0xC0FFEE ^ seed),
            speed: Speed::X4,
            step_once: false,
            heatmap: 0,
            log: vec!["Welcome to DungeonMind. The hero learns as you watch.".into()],
            floats: vec![],
            parts: vec![],
            shake: 0.0,
            cam: Vec2::ZERO,
            hist: Hist::default(),
            cur_q: [0.0; N_ACTIONS],
            cur_legal: [false; N_ACTIONS],
            cur_action: 0,
            ep_reward: 0.0,
            ep_turns: 0,
            episodes_total: 0,
            seed_fixed: fixed,
            cur_seed: seed,
            save_ctr: 0,
            time: 0.0,
            fps_fast: 0,
            sim_acc: 0.0,
            last_save: 0.0,
            zoom: 1.15,
            zoom_target: 1.15,
            follow: true,
            watch: false,
            thought_on: true,
            trainer: None,
            showcase: None,
            recap: None,
            sc_pause: 0.0,
            sc_no: 0,
            hero_anim: Anim::at(0, 0),
            mon_anim: vec![],
            anim_key: (u64::MAX, 0),
            dice: vec![],
            slowmo: 0.0,
            hint_t: 0.0,
            cur_why: String::new(),
            hist_stride: 1,
            hist_ctr: 0,
            last_speed: Speed::Q50,
            last_click: -10.0,
            drag_on: false,
            drag_moved: false,
            drag_start: Vec2::ZERO,
            drag_last: Vec2::ZERO,
            sc_floor_seen: 0,
        }
    }

    /// Append one episode to the chart history (decimated so it stays bounded
    /// even when the background trainer finishes hundreds of episodes/s).
    pub fn push_hist(&mut self, st: &EpStats) {
        self.hist_ctr += 1;
        if self.hist_ctr % self.hist_stride != 0 {
            return;
        }
        self.hist.rew.push(st.reward);
        let n = self.hist.rew.len().min(100);
        let ma = self.hist.rew[self.hist.rew.len() - n..].iter().sum::<f32>() / n as f32;
        self.hist.ma.push(ma);
        self.hist.floor.push(st.floor as f32);
        self.hist.eps.push(st.eps);
        self.hist.qsz.push(st.q_size as f32);
        self.hist.win.push(if st.won { 1.0 } else { 0.0 });
        if self.hist.rew.len() > 3000 {
            let h = &mut self.hist;
            for v in [&mut h.rew, &mut h.ma, &mut h.floor, &mut h.eps, &mut h.qsz, &mut h.win] {
                let mut i = 0;
                v.retain(|_| {
                    i += 1;
                    i % 2 == 0
                });
            }
            self.hist_stride *= 2;
        }
    }

    pub fn record_episode(&mut self, st: &EpStats) {
        self.push_hist(st);
        self.agent.end_episode();
        self.episodes_total = self.agent.episodes;
        self.log.push(format!(
            "— Episode {}: reward {:.0}, floor {}, {} turns —",
            self.agent.episodes,
            st.reward,
            st.floor,
            st.turns
        ));
        self.trim_log();
        self.floats.clear();
        self.parts.clear();
        self.save_ctr += 1;
        if self.save_ctr % 10 == 0 {
            self.agent.save(&train::qtable_path()).ok();
        }
    }

    /// one hero turn with learning; true if the episode ended.
    pub fn run_turn(&mut self) -> bool {
        let obs = self.world.build_obs();
        let s = obs.key();
        let legal = self.world.legal();
        let a = self.agent.choose(s, &legal, &mut self.ep_rng);
        self.cur_why = watch::why_text(&self.world, &obs, a);
        self.cur_action = a;
        self.cur_legal = legal;
        self.cur_q = self.agent.qvals(s);
        let (mut r, done) = self.world.step(a);
        if self.world.dead {
            r += R_DEATH;
        }
        let obs2 = self.world.build_obs();
        self.agent.update(s, a, r, obs2.key(), done);
        self.ep_reward += r;
        self.ep_turns += 1;
        if !self.speed.is_fast() {
            self.drain_events();
        } else {
            self.world.events.clear(); // fast-forward: skip FX generation
        }
        if done {
            let st = EpStats {
                reward: self.ep_reward,
                turns: self.ep_turns,
                floor: self.world.deepest_floor(),
                won: self.world.won,
                dead: self.world.dead,
                q_size: self.agent.q.len(),
                eps: self.agent.eps,
            };
            self.record_episode(&st);
            self.ep_reward = 0.0;
            self.ep_turns = 0;
            let seed = if self.seed_fixed {
                self.cur_seed
            } else {
                self.ep_rng.next_u64()
            };
            self.cur_seed = seed;
            self.world = World::new(seed);
            self.drain_events();
            return true;
        }
        false
    }

    pub fn action_name(a: usize) -> &'static str {
        match a {
            0 => "move N",
            1 => "move NE",
            2 => "move E",
            3 => "move SE",
            4 => "move S",
            5 => "move SW",
            6 => "move W",
            7 => "move NW",
            A_ATTACK => "attack",
            A_QUAFF => "quaff potion",
            A_PICKUP => "pick up",
            A_OPEN => "open",
            _ => "descend",
        }
    }

    pub fn reset_learning(&mut self) {
        let was_watch = self.watch;
        if was_watch {
            self.set_watch(false);
        }
        self.agent.reset_learning();
        self.hist = Hist::default();
        self.episodes_total = 0;
        self.log.push("Learning reset — Q-table wiped.".into());
        self.trim_log();
        std::fs::remove_file(train::qtable_path()).ok();
        if was_watch {
            self.set_watch(true);
        }
    }

    pub fn new_seed(&mut self) {
        let s = self.ep_rng.next_u64();
        self.cur_seed = s;
        self.cfg.seed = s;
        if self.watch {
            self.start_showcase(Some(s));
            return;
        }
        self.world = World::new(s);
        self.ep_reward = 0.0;
        self.ep_turns = 0;
        self.log.push(format!("New dungeon, seed {}.", s));
        self.trim_log();
        self.drain_events();
    }

    pub fn save_now(&self) {
        let path = train::qtable_path();
        if let Some(tr) = &self.trainer {
            // snapshot is immutable: write it from a helper thread, atomically
            static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            let snap = tr.latest();
            std::thread::spawn(move || {
                let tmp = path.with_extension("tmp");
                if snap.save(&tmp).is_ok() {
                    std::fs::rename(&tmp, &path).ok();
                }
                BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
            });
        } else {
            self.agent.save(&path).ok();
        }
    }

    fn spawn_parts(&mut self, kind: u8, tx: f32, ty: f32) {
        let mut rng = Rng::new(
            (tx as i64 as u64).wrapping_mul(0x9E37) ^ (ty as i64 as u64).wrapping_mul(0x85EB) ^ (self.time as i64 as u64),
        );
        let (cols, n, spd, grav, life, size) = match kind {
            0 => (vec![Color::new(1.0, 0.9, 0.4, 1.0), Color::new(1.0, 1.0, 0.8, 1.0)], 7, 70.0, 90.0, 0.45, 2.5),
            1 => (vec![Color::new(0.6, 0.05, 0.08, 1.0), Color::new(0.35, 0.02, 0.04, 1.0)], 9, 55.0, 220.0, 0.6, 3.0),
            2 => (vec![Color::new(0.45, 0.55, 0.4, 1.0), Color::new(0.3, 0.4, 0.3, 1.0)], 8, 50.0, 200.0, 0.7, 3.5),
            3 => (vec![Color::new(0.4, 0.8, 1.0, 1.0), Color::new(0.7, 0.5, 1.0, 1.0)], 10, 45.0, -30.0, 0.8, 2.5),
            4 => (vec![Color::new(0.5, 0.45, 0.4, 0.7), Color::new(0.35, 0.32, 0.3, 0.6)], 5, 25.0, -15.0, 0.9, 4.0),
            _ => (vec![AMBER, Color::new(1.0, 0.9, 0.4, 1.0)], 8, 60.0, 160.0, 0.7, 2.5),
        };
        for _ in 0..n {
            let ang = rng.next_f32() * std::f32::consts::TAU;
            let sp = spd * (0.4 + rng.next_f32() * 0.8);
            self.parts.push(Part {
                x: tx,
                y: ty,
                vx: ang.cos() * sp,
                vy: ang.sin() * sp,
                life,
                max: life,
                col: cols[rng.range(0, cols.len())],
                grav,
                size,
            });
        }
    }

    pub fn drain_events(&mut self) {
        let evs = std::mem::take(&mut self.world.events);
        self.handle_events(evs);
    }

    pub fn handle_events(&mut self, evs: Vec<Event>) {
        for ev in evs {
            match ev {
                Event::Log(s) => {
                    self.log.push(s);
                    self.trim_log();
                }
                Event::Damage { x, y, amount, crit, heal } => {
                    let col = if heal {
                        Color::new(0.3, 0.9, 0.4, 1.0)
                    } else if crit {
                        Color::new(1.0, 0.35, 0.15, 1.0)
                    } else {
                        Color::new(1.0, 0.95, 0.85, 1.0)
                    };
                    let sz = if crit { 26.0 } else { 18.0 };
                    if !heal {
                        if (x, y) == (self.world.hero.x, self.world.hero.y) {
                            self.hero_anim.flash = 1.0;
                        } else if let Some(mi) = self.world.monster_at(x, y) {
                            if let Some(a) = self.mon_anim.get_mut(mi) {
                                a.flash = 1.0;
                            }
                        }
                    }
                    self.floats.push(FloatText {
                        x: x as f32 * TILE + TILE / 2.0,
                        y: y as f32 * TILE,
                        vy: -34.0,
                        life: 1.4,
                        txt: if heal { format!("+{}", amount) } else { format!("{}", amount) },
                        col,
                        size: sz,
                    });
                    if !heal {
                        self.spawn_parts(0, x as f32 * TILE + TILE / 2.0, y as f32 * TILE + TILE / 2.0);
                    }
                }
                Event::Shake(a) => self.shake = self.shake.max(a),
                Event::Part { kind, x, y } => {
                    self.spawn_parts(kind, x as f32 * TILE + TILE / 2.0, y as f32 * TILE + TILE / 2.0)
                }
                Event::FloorChanged(f) => {
                    self.log.push(format!("You enter floor {}.", f + 1));
                    self.trim_log();
                }
                Event::Die => {
                    self.shake = 14.0;
                    if !self.speed.is_fast() {
                        self.slowmo = 1.6;
                    }
                }
                Event::Roll { ax, ay, tx, ty, d20, bonus, ac, hit, hero } => {
                    let dir = Vec2::new((tx - ax) as f32, (ty - ay) as f32).normalize_or_zero();
                    let lunge = Some(Lunge { dir, t: 0.0, dur: 0.34 });
                    if hero {
                        self.hero_anim.lunge = lunge;
                    } else if let Some(mi) = self.world.monster_at(ax, ay) {
                        if let Some(a) = self.mon_anim.get_mut(mi) {
                            a.lunge = lunge;
                        }
                        let boss = self.world.monsters.get(mi).map(|m| m.is_boss).unwrap_or(false);
                        if boss && hit {
                            self.shake = self.shake.max(10.0);
                            if !self.speed.is_fast() {
                                self.slowmo = 1.2;
                            }
                        }
                    }
                    if self.dice.len() > 6 {
                        self.dice.remove(0);
                    }
                    self.dice.push(Dice { x: tx as f32 * TILE + TILE / 2.0, y: ty as f32 * TILE, life: 1.9, d20, bonus, ac, hit, hero });
                }
                Event::Win => self.log.push("VICTORY! The dungeon is conquered!".into()),
                Event::LevelUp => {
                    let (hx, hy) = (self.world.hero.x as f32 * TILE + TILE / 2.0, self.world.hero.y as f32 * TILE + TILE / 2.0);
                    self.spawn_parts(3, hx, hy);
                }
            }
        }
    }

    fn trim_log(&mut self) {
        while self.log.len() > 300 {
            self.log.remove(0);
        }
    }
}

include!("gui_draw.rs");
