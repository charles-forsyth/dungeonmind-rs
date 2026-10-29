use crate::rng::Rng;
use std::collections::HashMap;

pub const N_ACTIONS: usize = 13;
// 0..=7 moves in dir order N,NE,E,SE,S,SW,W,NW; 8 attack; 9 quaff potion;
// 10 pickup; 11 open door/chest; 12 descend stairs.
pub const A_ATTACK: usize = 8;
pub const A_QUAFF: usize = 9;
pub const A_PICKUP: usize = 10;
pub const A_OPEN: usize = 11;
pub const A_DESCEND: usize = 12;
pub const DIR_NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
pub const DIRS: [(i32, i32); 8] = [
    (0, -1),
    (1, -1),
    (1, 0),
    (1, 1),
    (0, 1),
    (-1, 1),
    (-1, 0),
    (-1, -1),
];

/// Terrain codes for the 8-neighbour ring (see DECISIONS.md).
pub const T_SOLID: u8 = 0; // wall / unknown / liquid
pub const T_FLOOR: u8 = 1;
pub const T_DOOR: u8 = 2; // closed door (locked or not)
pub const T_DOOR_OPEN: u8 = 3;
pub const T_STAIRS: u8 = 4;
pub const T_TRAP: u8 = 5;
pub const T_CHEST: u8 = 6;
pub const T_FOUNTAIN: u8 = 7;
pub const T_NONE: u8 = 8; // "no such thing nearby" for dir slots

/// Compact observation the simulator builds each turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obs {
    pub nbr: [u8; 8],
    pub mon_dir: u8,  // 0..7 or 8 none
    pub mon_dist: u8, // bucket 0..3: 1 / 2-3 / 4-6 / 7+
    pub hp: u8,       // bucket 0..7
    pub has_key: bool,
    pub door_dir: u8,
    pub quest_dir: u8,
    pub quest_dist: u8,
    pub stairs_dir: u8,
    pub stairs_dist: u8,
    pub has_potion: bool,
    pub on_stairs: bool,
    pub stuck: u8, // bucket 0..3: turns since last new tile seen
    pub last_action: u8,
}

impl Obs {
    pub fn blank() -> Obs {
        Obs {
            nbr: [T_SOLID; 8],
            mon_dir: T_NONE,
            mon_dist: 0,
            hp: 0,
            has_key: false,
            door_dir: T_NONE,
            quest_dir: T_NONE,
            quest_dist: 0,
            stairs_dir: T_NONE,
            stairs_dist: 0,
            has_potion: false,
            on_stairs: false,
            stuck: 0,
            last_action: 0,
        }
    }
    /// Pack into a u64 key for the table. 58 bits used.
    pub fn key(&self) -> u64 {
        let mut k: u64 = 0;
        let mut put = |bits: u64, n: u32| {
            k = (k << n) | (bits & ((1u64 << n) - 1));
        };
        for i in (0..8).rev() {
            put(self.nbr[i] as u64, 3);
        } // 24
        put(self.mon_dir as u64, 4);
        put(self.mon_dist as u64, 2);
        put(self.hp as u64, 3);
        put(self.has_key as u64, 1);
        put(self.door_dir as u64, 4);
        put(self.quest_dir as u64, 4);
        put(self.quest_dist as u64, 2);
        put(self.stairs_dir as u64, 4);
        put(self.stairs_dist as u64, 2);
        put(self.has_potion as u64, 1);
        put(self.on_stairs as u64, 1);
        put(self.stuck as u64, 2);
        put(self.last_action as u64, 4);
        k
    }
}

#[derive(Clone)]
pub struct QAgent {
    pub q: HashMap<u64, [f32; N_ACTIONS]>,
    pub alpha: f32,
    pub gamma: f32,
    pub eps: f32,
    pub eps_min: f32,
    pub eps_decay: f32,
    pub episodes: u32,
}

impl QAgent {
    pub fn new() -> QAgent {
        QAgent {
            q: HashMap::new(),
            alpha: env_f32("DM_ALPHA", 0.25),
            gamma: env_f32("DM_GAMMA", 0.97),
            eps: env_f32("DM_EPS", 1.0),
            eps_min: 0.02,
            eps_decay: env_f32("DM_EPS_DECAY", 0.995),
            episodes: 0,
        }
    }
    pub fn row(&mut self, s: u64) -> &mut [f32; N_ACTIONS] {
        self.q.entry(s).or_insert([0.0; N_ACTIONS])
    }
    pub fn qvals(&self, s: u64) -> [f32; N_ACTIONS] {
        self.q.get(&s).copied().unwrap_or([0.0; N_ACTIONS])
    }
    /// Epsilon-greedy over legal actions only.
    pub fn choose(&mut self, s: u64, legal: &[bool; N_ACTIONS], rng: &mut Rng) -> usize {
        let vals = self.qvals(s);
        let mut best: Vec<usize> = (0..N_ACTIONS).filter(|&a| legal[a]).collect();
        if best.is_empty() {
            return A_PICKUP; // never happens in practice (wait-ish fallback)
        }
        if rng.next_f32() < self.eps {
            return best[rng.range(0, best.len())];
        }
        let mut bv = f32::NEG_INFINITY;
        for &a in &best {
            if vals[a] > bv {
                bv = vals[a];
            }
        }
        best.retain(|&a| (vals[a] - bv).abs() < 1e-9);
        best[rng.range(0, best.len())]
    }
    /// Read-only policy for showcase play: epsilon-greedy over legal actions with an explicit eps.
    pub fn choose_with_eps(&self, s: u64, legal: &[bool; N_ACTIONS], rng: &mut Rng, eps: f32) -> usize {
        let vals = self.qvals(s);
        let mut best: Vec<usize> = (0..N_ACTIONS).filter(|&a| legal[a]).collect();
        if best.is_empty() {
            return A_PICKUP;
        }
        if rng.next_f32() < eps {
            return best[rng.range(0, best.len())];
        }
        let bv = best.iter().map(|&a| vals[a]).fold(f32::NEG_INFINITY, f32::max);
        best.retain(|&a| (vals[a] - bv).abs() < 1e-9);
        best[rng.range(0, best.len())]
    }
    /// The Q-learning update.
    pub fn update(&mut self, s: u64, a: usize, r: f32, s2: u64, done: bool) {
        let target = if done {
            r
        } else {
            r + self.gamma * self.qvals(s2).iter().cloned().fold(f32::NEG_INFINITY, f32::max)
        };
        let alpha = self.alpha;
        let row = self.row(s);
        row[a] += alpha * (target - row[a]);
    }
    pub fn end_episode(&mut self) {
        self.episodes += 1;
        self.eps = (self.eps * self.eps_decay).max(self.eps_min);
    }
    pub fn reset_learning(&mut self) {
        self.q.clear();
        self.episodes = 0;
        self.eps = env_f32("DM_EPS", 1.0);
    }
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let mut buf: Vec<u8> = Vec::with_capacity(4 + self.q.len() * (8 + N_ACTIONS * 4));
        buf.extend_from_slice(b"DMQ1");
        buf.extend_from_slice(&(self.q.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.episodes.to_le_bytes());
        buf.extend_from_slice(&self.eps.to_le_bytes());
        for (k, row) in &self.q {
            buf.extend_from_slice(&k.to_le_bytes());
            for v in row {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        std::fs::write(path, buf)
    }
    pub fn load(&mut self, path: &std::path::Path) -> bool {
        if let Ok(b) = std::fs::read(path) {
            if b.len() >= 20 && &b[0..4] == b"DMQ1" {
                let n = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
                self.episodes = u32::from_le_bytes(b[8..12].try_into().unwrap());
                self.eps = f32::from_le_bytes(b[12..16].try_into().unwrap());
                self.q.clear();
                let mut i = 16usize;
                let need = 8 + N_ACTIONS * 4;
                for _ in 0..n {
                    if i + need > b.len() {
                        break;
                    }
                    let k = u64::from_le_bytes(b[i..i + 8].try_into().unwrap());
                    let mut row = [0.0f32; N_ACTIONS];
                    for j in 0..N_ACTIONS {
                        row[j] = f32::from_le_bytes(b[i + 8 + j * 4..i + 12 + j * 4].try_into().unwrap());
                    }
                    self.q.insert(k, row);
                    i += need;
                }
                return true;
            }
        }
        false
    }
    pub fn table_size(&self) -> usize {
        self.q.len()
    }
}

impl Default for QAgent {
    fn default() -> Self {
        Self::new()
    }
}

fn env_f32(name: &str, def: f32) -> f32 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(def)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn q_update_matches_hand_computed() {
        let mut ag = QAgent::new();
        ag.alpha = 0.5;
        ag.gamma = 0.9;
        let s = 7u64;
        let s2 = 8u64;
        // seed max of s2 row = 2.0
        ag.row(s2)[3] = 2.0;
        // Q(s,a)=0, r=1 => target = 1 + 0.9*2 = 2.8; Q <- 0 + 0.5*(2.8-0) = 1.4
        ag.update(s, 1, 1.0, s2, false);
        assert!((ag.qvals(s)[1] - 1.4).abs() < 1e-5);
        // terminal: target = r only => Q <- 1.4 + 0.5*(-2 - 1.4) = -0.3
        ag.update(s, 1, -2.0, s2, true);
        assert!((ag.qvals(s)[1] - (-0.3)).abs() < 1e-5);
    }
    #[test]
    fn state_key_packing() {
        let a = Obs::blank();
        let mut b = a;
        assert_eq!(a.key(), b.key());
        b.nbr[2] = T_DOOR;
        assert_ne!(a.key(), b.key());
        b = a;
        b.hp = 3;
        assert_ne!(a.key(), b.key());
        b = a;
        b.last_action = 5;
        assert_ne!(a.key(), b.key());
        // every field flips at least one bit
        for (mut o, name) in [
            ({ let mut o = a; o.has_key = true; o }, "has_key"),
            ({ let mut o = a; o.on_stairs = true; o }, "on_stairs"),
            ({ let mut o = a; o.has_potion = true; o }, "has_potion"),
            ({ let mut o = a; o.mon_dir = 1; o }, "mon_dir"),
            ({ let mut o = a; o.stairs_dir = 2; o }, "stairs_dir"),
            ({ let mut o = a; o.quest_dir = 3; o }, "quest_dir"),
            ({ let mut o = a; o.stuck = 1; o }, "stuck"),
            ({ let mut o = a; o.mon_dist = 1; o }, "mon_dist"),
            ({ let mut o = a; o.quest_dist = 1; o }, "quest_dist"),
            ({ let mut o = a; o.stairs_dist = 1; o }, "stairs_dist"),
        ] {
            o.nbr[0] |= 0; // silence
            assert_ne!(a.key(), o.key(), "field {name} not encoded");
        }
    }
    #[test]
    fn choose_respects_mask_and_exploit() {
        let mut ag = QAgent::new();
        ag.eps = 0.0;
        let s = 99u64;
        ag.row(s)[A_OPEN] = 5.0;
        ag.row(s)[0] = 9.0; // best but illegal
        let mut legal = [true; N_ACTIONS];
        legal[0] = false;
        let mut rng = Rng::new(1);
        assert_eq!(ag.choose(s, &legal, &mut rng), A_OPEN);
    }
    #[test]
    fn save_load_roundtrip() {
        let mut ag = QAgent::new();
        ag.row(1)[2] = 3.5;
        ag.row(u64::MAX)[12] = -1.25;
        ag.episodes = 17;
        let p = std::env::temp_dir().join("dm_qtest.bin");
        ag.save(&p).unwrap();
        let mut ag2 = QAgent::new();
        assert!(ag2.load(&p));
        assert_eq!(ag2.qvals(1)[2], 3.5);
        assert_eq!(ag2.qvals(u64::MAX)[12], -1.25);
        assert_eq!(ag2.episodes, 17);
        assert_eq!(ag2.table_size(), 2);
        let _ = std::fs::remove_file(p);
    }
}
