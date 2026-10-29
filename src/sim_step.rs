// part 2 of sim: observation, legal actions, step. (included by sim.rs)
use super::*;

impl World {
    pub fn monster_at(&self, x: i32, y: i32) -> Option<usize> {
        self.monsters.iter().position(|m| m.hp > 0 && m.x == x && m.y == y)
    }

    pub fn trap_revealed(&self, x: usize, y: usize) -> bool {
        for (dx, dy) in [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < self.map.w && (ny as usize) < self.map.h {
                if self.visited[self.map.idx(nx as usize, ny as usize)] {
                    return true;
                }
            }
        }
        false
    }

    fn nbr_code(&self, x: i32, y: i32) -> u8 {
        let c = match self.map.at(x, y) {
            Some(c) => c,
            None => return T_SOLID,
        };
        match c.t {
            Tile::Wall | Tile::SecretDoor | Tile::Torch | Tile::Water | Tile::Lava => T_SOLID,
            Tile::Door | Tile::Vault => {
                if c.locked >= 0 {
                    T_DOOR
                } else {
                    T_DOOR_OPEN
                }
            }
            Tile::StairsDown => T_STAIRS,
            Tile::Chest => {
                if self.opened_chests.contains(&self.map.idx(x as usize, y as usize)) {
                    T_FLOOR
                } else {
                    T_CHEST
                }
            }
            Tile::Fountain => {
                if self.used_fountains.contains(&self.map.idx(x as usize, y as usize)) {
                    T_FLOOR
                } else {
                    T_FOUNTAIN
                }
            }
            Tile::Trap => {
                if self.trap_revealed(x as usize, y as usize) {
                    T_TRAP
                } else {
                    T_FLOOR
                }
            }
            _ => T_FLOOR,
        }
    }

    pub fn build_obs(&self) -> Obs {
        let mut o = Obs::blank();
        for (i, (dx, dy)) in DIRS.iter().enumerate() {
            o.nbr[i] = self.nbr_code(self.hero.x + dx, self.hero.y + dy);
        }
        // nearest visible monster
        let mut bd = i32::MAX;
        let mut bdir = T_NONE;
        for m in &self.monsters {
            if m.hp <= 0 {
                continue;
            }
            let i = self.map.idx(m.x as usize, m.y as usize);
            if !self.visible[i] {
                continue;
            }
            let (dx, dy) = (m.x - self.hero.x, m.y - self.hero.y);
            let d = dx.abs().max(dy.abs());
            if d < bd {
                bd = d;
                bdir = dir_of(dx, dy) as u8;
            }
        }
        o.mon_dir = bdir;
        o.mon_dist = dist_bucket(bd);
        // stairs compass: a "sense of home" BFS gradient (see DECISIONS.md)
        {
            let si = self.map.idx(self.map.stairs.0, self.map.stairs.1);
            let hx = self.hero.x.clamp(0, self.map.w as i32 - 1) as usize;
            let hy = self.hero.y.clamp(0, self.map.h as i32 - 1) as usize;
            let hd = self.dist_stairs[self.map.idx(hx, hy)];
            o.on_stairs = hd == 0;
            o.stairs_dist = if hd == u16::MAX {
                3
            } else if hd <= 1 {
                0
            } else if hd <= 8 {
                1
            } else if hd <= 30 {
                2
            } else {
                3
            };
            if hd > 0 {
                // pick the neighbour tile one step closer along the BFS field
                let mut bd = hd;
                let mut bdir = T_NONE;
                for dir in 0..8usize {
                    let (dx, dy) = DIRS[dir];
                    let nx = hx as i32 + dx;
                    let ny = hy as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= self.map.w as i32 || ny >= self.map.h as i32 {
                        continue;
                    }
                    let j = self.map.idx(nx as usize, ny as usize);
                    let t = self.map.cells[j].t;
                    if matches!(t, Tile::Wall | Tile::SecretDoor | Tile::Torch) {
                        continue;
                    }
                    let d = self.dist_stairs[j];
                    if d < bd {
                        bd = d;
                        bdir = dir as u8;
                    }
                }
                o.stairs_dir = bdir;
            } else {
                o.stairs_dir = T_NONE;
            }
            let _ = si;
        }
        // nearest visible locked door
        let mut bdd = i32::MAX;
        let mut bdi = T_NONE;
        let mut tier_of = usize::MAX;
        for y in 0..self.map.h {
            for x in 0..self.map.w {
                let i = self.map.idx(x, y);
                if !self.visible[i] {
                    continue;
                }
                let c = &self.map.cells[i];
                if matches!(c.t, Tile::Door | Tile::Vault) && c.locked >= 0 {
                    let (dx, dy) = (x as i32 - self.hero.x, y as i32 - self.hero.y);
                    let d = dx.abs().max(dy.abs());
                    if d < bdd {
                        bdd = d;
                        bdi = dir_of(dx, dy) as u8;
                        tier_of = c.locked as usize;
                    }
                }
            }
        }
        o.door_dir = bdi;
        o.has_key = tier_of < 3 && self.hero.has_key(tier_of);
        // quest target direction (first incomplete, if its tile is known)
        if let Some((tx, ty)) = self.quest_target() {
            let (dx, dy) = (tx as i32 - self.hero.x, ty as i32 - self.hero.y);
            o.quest_dir = dir_of(dx, dy) as u8;
            o.quest_dist = dist_bucket(dx.abs().max(dy.abs()));
        }
        let hpr = self.hero.hp.max(0) as f32 / self.hero.max_hp as f32;
        o.hp = ((hpr * 8.0) as u8).min(7);
        o.has_potion = self.hero.potions > 0 && self.hero.hp < self.hero.max_hp;
        let stuck = self.turn.saturating_sub(self.last_new);
        o.stuck = if stuck < 10 {
            0
        } else if stuck < 40 {
            1
        } else if stuck < 120 {
            2
        } else {
            3
        };
        o.last_action = self.last_action as u8;
        o
    }

    pub fn quest_target(&self) -> Option<(usize, usize)> {
        for (q, done) in &self.quests.quests {
            if *done {
                continue;
            }
            match q {
                Quest::FetchKey => {
                    if let Some((_, _, k)) = self.map.locked_doors.iter().find(|(t, _, _)| *t == 2) {
                        return Some(*k);
                    }
                }
                Quest::OpenVault => {
                    if let Some(v) = self.quests.vault_spot {
                        return Some(v);
                    }
                }
                Quest::Rescue => {
                    if let Some(c) = self.quests.captive_spot {
                        return Some(c);
                    }
                }
                Quest::Shrine => {
                    if let Some(s) = self.quests.shrine_spot {
                        return Some(s);
                    }
                }
                Quest::SlayBoss => {
                    if let Some(m) = self.monsters.iter().find(|m| m.is_boss && m.hp > 0) {
                        return Some((m.x as usize, m.y as usize));
                    }
                }
            }
        }
        None
    }

    pub fn legal(&self) -> [bool; N_ACTIONS] {
        let mut l = [false; N_ACTIONS];
        for a in 0..8 {
            let (dx, dy) = DIRS[a];
            let (nx, ny) = (self.hero.x + dx, self.hero.y + dy);
            let c = match self.map.at(nx, ny) {
                Some(c) => c,
                None => continue,
            };
            if !self.map.passable(nx, ny) {
                continue;
            }
            if c.locked >= 0 && !self.hero.has_key(c.locked as usize) {
                continue;
            }
            if self.monster_at(nx, ny).is_some() {
                continue;
            }
            l[a] = true;
        }
        l[A_ATTACK] = (0..8).any(|a| {
            let (dx, dy) = DIRS[a];
            self.monster_at(self.hero.x + dx, self.hero.y + dy).is_some()
        });
        l[A_QUAFF] = self.hero.potions > 0 && self.hero.hp < self.hero.max_hp;
        l[A_PICKUP] = self.items.iter().any(|(ix, iy, _)| {
            (*ix as i32 - self.hero.x).abs() <= 1 && (*iy as i32 - self.hero.y).abs() <= 1
        });
        // open: adjacent/on chest, door, fountain
        let mut can_open = false;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (nx, ny) = (self.hero.x + ox, self.hero.y + oy);
                if let Some(c) = self.map.at(nx, ny) {
                    let i = self.map.idx(nx as usize, ny as usize);
                    match c.t {
                        Tile::Door | Tile::Vault => {
                            if c.locked >= 0 && !self.hero.has_key(c.locked as usize) {
                                continue;
                            }
                            can_open = true;
                        }
                        Tile::Chest if !self.opened_chests.contains(&i) => can_open = true,
                        Tile::Fountain if !self.used_fountains.contains(&i) && ox == 0 && oy == 0 => {
                            can_open = true
                        }
                        _ => {}
                    }
                }
            }
        }
        l[A_OPEN] = can_open;
        l[A_DESCEND] = matches!(
            self.map.at(self.hero.x, self.hero.y).map(|c| c.t),
            Some(Tile::StairsDown)
        );
        l
    }
}

pub fn dir_of(dx: i32, dy: i32) -> usize {
    if dx > 0 && dy < 0 {
        1
    } else if dx > 0 && dy == 0 {
        2
    } else if dx > 0 && dy > 0 {
        3
    } else if dx == 0 && dy > 0 {
        4
    } else if dx < 0 && dy > 0 {
        5
    } else if dx < 0 && dy == 0 {
        6
    } else if dx < 0 && dy < 0 {
        7
    } else {
        0
    }
}
fn dist_bucket(d: i32) -> u8 {
    if d <= 1 {
        0
    } else if d <= 3 {
        1
    } else if d <= 6 {
        2
    } else {
        3
    }
}

include!("sim_act.rs");
