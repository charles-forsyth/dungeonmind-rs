use crate::dungeon::*;
use crate::entities::*;
use crate::fov;
use crate::qlearn::*;
use crate::rng::Rng;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ItemKind {
    Potion,
    Key(usize),
    Gold,
}

#[derive(Clone, Debug)]
pub enum Event {
    Damage { x: i32, y: i32, amount: i32, crit: bool, heal: bool },
    Log(String),
    Shake(f32),
    Part { kind: u8, x: i32, y: i32 }, // 0 spark 1 blood 2 slime 3 magic 4 dust 5 gold
    FloorChanged(usize),
    Die,
    Win,
    LevelUp,
    /// d20 attack roll (for the GUI dice display / lunges).
    Roll { ax: i32, ay: i32, tx: i32, ty: i32, d20: i32, bonus: i32, ac: i32, hit: bool, hero: bool },
}

pub struct World {
    pub seed: u64,
    pub floor_no: usize,
    pub map: FloorGen,
    pub explored: Vec<bool>,
    pub visible: Vec<bool>,
    pub visited: Vec<bool>,
    pub visits: Vec<u16>, // per-cell step counts (heatmap)
    pub monsters: Vec<Monster>,
    pub items: Vec<(usize, usize, ItemKind)>,
    pub hero: Hero,
    pub quests: QuestState,
    pub rng: Rng,
    pub turn: u32,
    pub dead: bool,
    pub won: bool,
    pub events: Vec<Event>,
    pub opened_chests: HashSet<usize>,
    pub used_fountains: HashSet<usize>,
    pub last_new: u32,   // turn of last newly-visited tile
    pub elite_alive: bool,
    pub light_radius: i32,
    pub last_action: usize,
    pub last_obs_key: u64,
    pub turns_this_floor: u32,
    pub visited_ct: u32,
    pub dist_stairs: Vec<u16>, // BFS dist (walkable) to stairs, 0xFFFF = unreachable
    pub recent: std::collections::VecDeque<usize>, // last positions for anti-loop shaping
}

const MAX_TURNS: u32 = 4000;

pub const R_NEW_TILE: f32 = 0.10;
pub const R_PICKUP: f32 = 2.0;
pub const R_KEY: f32 = 12.0;
pub const R_DOOR: f32 = 6.0;
pub const R_QUEST: f32 = 40.0;
pub const R_KILL_BASE: f32 = 2.0;
pub const R_DESCEND: f32 = 70.0;
pub const R_HIT_TAKEN: f32 = -2.0;
pub const R_WASTE: f32 = -0.05;
pub const R_DEATH: f32 = -60.0;
pub const R_WIN: f32 = 250.0;

impl World {
    pub fn new(seed: u64) -> World {
        let mut w = World {
            seed,
            floor_no: 0,
            map: gen_floor(seed, 0),
            explored: vec![],
            visible: vec![],
            visited: vec![],
            visits: vec![],
            monsters: vec![],
            items: vec![],
            hero: Hero::new(1, 1),
            quests: QuestState {
                quests: vec![],
                key_found: false,
                captive_spot: None,
                shrine_spot: None,
                vault_spot: None,
                boss_alive: false,
            },
            rng: Rng::new(seed ^ 0xA5A5),
            turn: 0,
            dead: false,
            won: false,
            events: vec![],
            opened_chests: HashSet::new(),
            used_fountains: HashSet::new(),
            last_new: 0,
            visited_ct: 0,
            dist_stairs: vec![],
            recent: std::collections::VecDeque::new(),
            elite_alive: false,
            light_radius: 9,
            last_action: 0,
            last_obs_key: 0,
            turns_this_floor: 0,
        };
        w.enter_floor(0);
        w
    }

    fn enter_floor(&mut self, floor: usize) {
        self.floor_no = floor;
        self.map = gen_floor(self.seed, floor);
        let n = self.map.cells.len();
        self.explored = vec![false; n];
        self.visible = vec![false; n];
        self.visited = vec![false; n];
        self.visits = vec![0u16; n];
        self.monsters.clear();
        self.items.clear();
        self.opened_chests.clear();
        self.used_fountains.clear();
        self.turns_this_floor = 0;
        self.visited_ct = 0;
        self.build_stair_dist();
        let (ex, ey) = if floor == 0 {
            self.map.entry
        } else {
            self.map.entry
        };
        self.hero.x = ex as i32;
        self.hero.y = ey as i32;
        let mut rng = Rng::new(self.seed ^ (floor as u64).wrapping_mul(0xBEEF) ^ 0xC0FFEE);
        // quests
        self.quests = pick_quests(floor, &self.map, &mut rng);
        // ground keys for every locked door
        for (tier, _door, key) in self.map.locked_doors.clone() {
            self.items.push((key.0, key.1, ItemKind::Key(tier)));
        }
        // extra potions / gold
        let n_p = rng.range(2, 4);
        for _ in 0..n_p {
            if let Some(p) = self.random_open_spot(&mut rng, 8) {
                self.items.push((p.0, p.1, ItemKind::Potion));
            }
        }
        let n_g = rng.range(2, 5);
        for _ in 0..n_g {
            if let Some(p) = self.random_open_spot(&mut rng, 8) {
                self.items.push((p.0, p.1, ItemKind::Gold));
            }
        }
        // monsters: count scales with floor
        let n_m = 10 + floor * 3;
        let pool: Vec<Kind> = match floor {
            0 => vec![Kind::Rat, Kind::Bat, Kind::Goblin],
            1 => vec![Kind::Rat, Kind::Goblin, Kind::SkeletonArcher, Kind::SlimeBig],
            2 => vec![Kind::Goblin, Kind::SkeletonArcher, Kind::SlimeBig, Kind::Cultist, Kind::Mimic],
            _ => {
                let mut v = vec![Kind::Goblin, Kind::SkeletonArcher, Kind::Cultist, Kind::SlimeBig, Kind::Wraith, Kind::Ogre, Kind::Mimic, Kind::Bat];
                if floor >= 5 {
                    v.push(Kind::Lich);
                }
                v
            }
        };
        let mut placed = 0;
        let mut guard = 0;
        while placed < n_m && guard < 4000 {
            guard += 1;
            if let Some((x, y)) = self.random_open_spot(&mut rng, 16) {
                let k = pool[rng.range(0, pool.len())];
                // keep mimics glued to chest tiles
                if k == Kind::Mimic {
                    continue;
                }
                self.monsters.push(k.spawn(x as i32, y as i32, floor, &mut rng));
                placed += 1;
            }
        }
        for cp in self.map.chest_positions.clone() {
            if rng.chance(0.18) {
                let mut m = Kind::Mimic.spawn(cp.0 as i32, cp.1 as i32, floor, &mut rng);
                m.name = "Chest".to_string();
                self.monsters.push(m);
            }
        }
        // named elite for slay quest (boss on boss floors)
        let (elite_kind, elite_name) = match (floor, is_boss_floor(floor)) {
            (7, _) => (Kind::Dragon, "Vorgarax the Ancient Dragon".to_string()),
            (5, true) => (Kind::Lich, "Szintar the Lich".to_string()),
            (3, true) => (Kind::Wraith, "Malefor the Wraith King".to_string()),
            (1, true) => (Kind::Ogre, "Grukk Twoblades".to_string()),
            _ => (
                if floor >= 3 { Kind::Ogre } else { Kind::Goblin },
                format!(
                    "{} the Vile",
                    if floor >= 3 { "Boggluk" } else { "Snikkit" }
                ),
            ),
        };
        if let Some((x, y)) = self.random_open_spot(&mut rng, 20) {
            let mut e = elite_kind.spawn(x as i32, y as i32, floor, &mut rng);
            e.hp *= 2;
            e.max_hp *= 2;
            e.name = elite_name;
            e.is_boss = is_boss_floor(floor) || floor == 7;
            self.monsters.push(e);
            self.elite_alive = true;
        }
        self.update_fov();
        self.mark_seen();
        self.events.push(Event::FloorChanged(floor));
    }

    fn random_open_spot(&self, rng: &mut Rng, min_dist: usize) -> Option<(usize, usize)> {
        for _ in 0..200 {
            let x = rng.range(1, self.map.w - 1);
            let y = rng.range(1, self.map.h - 1);
            if !self.map.passable(x as i32, y as i32) {
                continue;
            }
            let c = &self.map.cells[self.map.idx(x, y)];
            if !matches!(c.t, Tile::Floor | Tile::Cave | Tile::Corridor) {
                continue;
            }
            if x.abs_diff(self.hero.x as usize) + y.abs_diff(self.hero.y as usize) < min_dist {
                continue;
            }
            if self.monsters.iter().any(|m| m.x as usize == x && m.y as usize == y) {
                continue;
            }
            if self.items.iter().any(|(ix, iy, _)| *ix == x && *iy == y) {
                continue;
            }
            return Some((x, y));
        }
        None
    }

    pub fn solid_for_sight(&self, x: i32, y: i32) -> bool {
        match self.map.at(x, y) {
            Some(c) => !matches!(
                c.t,
                Tile::Floor
                    | Tile::Cave
                    | Tile::Corridor
                    | Tile::Door
                    | Tile::StairsDown
                    | Tile::Fountain
                    | Tile::Chest
                    | Tile::Trap
                    | Tile::Shrine
            ),
            None => true,
        }
    }

    pub fn update_fov(&mut self) {
        let r = self.light_radius;
        let w = self.map.w;
        let h = self.map.h;
        // snapshot solidity so the closure doesn't borrow self while we mutate visible
        let solid_map: Vec<bool> = self
            .map
            .cells
            .iter()
            .map(|c| {
                !matches!(
                    c.t,
                    Tile::Floor
                        | Tile::Cave
                        | Tile::Corridor
                        | Tile::Door
                        | Tile::StairsDown
                        | Tile::Fountain
                        | Tile::Chest
                        | Tile::Trap
                        | Tile::Shrine
                )
            })
            .collect();
        let (hx, hy) = (self.hero.x, self.hero.y);
        let solid = |x: i32, y: i32| -> bool {
            if x < 0 || y < 0 || x as usize >= w || y as usize >= h {
                return true;
            }
            solid_map[y as usize * w + x as usize]
        };
        for v in self.visible.iter_mut() {
            *v = false;
        }
        let vis = &mut self.visible;
        let expl = &mut self.explored;
        compute_fov_into(w, h, hx, hy, r, &solid, vis, expl);
    }

    /// BFS distance-to-stairs over walkable tiles (locked doors & vaults treated as passable)
    pub fn build_stair_dist(&mut self) {
        let w = self.map.w;
        let h = self.map.h;
        let n = w * h;
        let mut d = vec![u16::MAX; n];
        let start = self.map.stairs.1 * w + self.map.stairs.0;
        d[start] = 0;
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        queue.push_back(start);
        let walk = |t: Tile| {
            !matches!(t, Tile::Wall | Tile::SecretDoor | Tile::Torch | Tile::Lava)
        };
        while let Some(i) = queue.pop_front() {
            let x = i % w;
            let y = i / w;
            let nd = d[i] + 1;
            for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let j = ny as usize * w + nx as usize;
                if d[j] == u16::MAX && walk(self.map.cells[j].t) {
                    d[j] = nd.min(u16::MAX - 1);
                    queue.push_back(j);
                }
            }
        }
        self.dist_stairs = d;
    }

    pub fn stair_pot(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 {
            return 0.0;
        }
        let i = self.map.idx(x as usize, y as usize);
        match self.dist_stairs.get(i) {
            Some(&u16::MAX) | None => 0.0,
            Some(&v) => -(v as f32),
        }
    }

    /// reveal traps adjacent to visited tiles + mark visited
    pub fn mark_seen(&mut self) {
        let i = self.map.idx(self.hero.x as usize, self.hero.y as usize);
        if self.visited[i] {
            // still count re-entry for heatmap
        } else {
            self.visited[i] = true;
            self.last_new = self.turn;
            self.visited_ct += 1;
        }
        self.visits[i] = self.visits[i].saturating_add(1);
    }
}

fn compute_fov_into(
    w: usize,
    h: usize,
    cx: i32,
    cy: i32,
    r: i32,
    solid: &impl Fn(i32, i32) -> bool,
    visible: &mut [bool],
    explored: &mut [bool],
) {
    fov::compute_fov(w, h, cx, cy, r, solid, &mut |x, y| {
        visible[y * w + x] = true;
        explored[y * w + x] = true;
    });
}

include!("sim_step.rs");
