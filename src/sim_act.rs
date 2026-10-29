// part 3 of sim: applying actions, rewards, monster AI entry. (included by sim_step.rs)
use super::*;

impl World {
    /// Apply one hero action + monster phase. Returns (reward, done).
    pub fn step(&mut self, action: usize) -> (f32, bool) {
        if self.dead || self.won {
            return (0.0, true);
        }
        let legal = self.legal();
        let mut r = R_WASTE;
        self.turn += 1;
        self.turns_this_floor += 1;
        self.last_action = action;
        if !legal[action] {
            // illegal action attempted (should be masked by agent): small penalty
            return (R_WASTE - 0.5, self.check_done());
        }
        match action {
            0..=7 => {
                let (dx, dy) = DIRS[action];
                let (nx, ny) = (self.hero.x + dx, self.hero.y + dy);
                let pot0 = self.stair_pot(self.hero.x, self.hero.y);
                self.hero.x = nx;
                self.hero.y = ny;
                let pot1 = self.stair_pot(nx, ny);
                if pot0 != 0.0 && pot1 != 0.0 {
                    r += 0.5 * (pot1 - pot0).clamp(-2.0, 2.0);
                }
                let ni = self.map.idx(nx as usize, ny as usize);
                if self.recent.contains(&ni) {
                    r -= 0.3; // punish short-horizon oscillation
                }
                if self.recent.len() >= 12 {
                    self.recent.pop_front();
                }
                self.recent.push_back(ni);
                let i = self.map.idx(nx as usize, ny as usize);
                if !self.visited[i] {
                    let novelty = (1.0 - self.visited_ct as f32 / 850.0).max(0.05);
                    r += R_NEW_TILE * novelty;
                }
                if self.turns_this_floor > 500 {
                    r -= 0.02; // pressure to find the stairs, not camp
                }
                self.mark_seen();
                let (ct, armed) = (self.map.cells[i].t, self.map.cells[i].trap_armed);
                if ct == Tile::Trap && armed {
                    self.map.set(nx as usize, ny as usize).trap_armed = false;
                    let dmg = roll(1, 4 + self.floor_no as u32, &mut self.rng);
                    self.hero.hp -= dmg;
                    r += R_HIT_TAKEN;
                    self.events.push(Event::Damage { x: nx, y: ny, amount: dmg, crit: false, heal: false });
                    self.events.push(Event::Part { kind: 1, x: nx, y: ny });
                    self.events.push(Event::Shake(4.0));
                    self.events.push(Event::Log(format!("A hidden trap snaps for {} damage!", dmg)));
                }
                if ct == Tile::Fountain && !self.used_fountains.contains(&i) {
                    self.used_fountains.insert(i);
                    self.hero.hp = self.hero.max_hp;
                    self.events.push(Event::Damage { x: nx, y: ny, amount: self.hero.max_hp, crit: false, heal: true });
                    self.events.push(Event::Part { kind: 3, x: nx, y: ny });
                    self.events.push(Event::Log("A healing fountain restores you fully.".into()));
                }
                if ct == Tile::Shrine {
                    if self.complete_quest(&Quest::Shrine) {
                        r += R_QUEST;
                        self.events.push(Event::Log("The forgotten shrine glows — blessed!".into()));
                    }
                }
            }
            A_ATTACK => r += self.hero_attack(),
            A_QUAFF => {
                self.hero.potions -= 1;
                let h = roll(2, 8, &mut self.rng) + 4;
                self.hero.hp = (self.hero.hp + h).min(self.hero.max_hp);
                self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: h, crit: false, heal: true });
                self.events.push(Event::Log(format!("You quaff a potion (+{} hp).", h)));
            }
            A_PICKUP => r += self.pickup(),
            A_OPEN => r += self.open(),
            A_DESCEND => {
                r += R_DESCEND;
                if self.floor_no + 1 >= N_FLOORS {
                    self.won = true;
                    r += R_WIN;
                    self.events.push(Event::Win);
                    self.events.push(Event::Log("You escaped the dungeon. Victory!".into()));
                    return (r, true);
                }
                self.enter_floor(self.floor_no + 1);
                let rec = self.hero.max_hp / 3;
                self.hero.hp = (self.hero.hp + rec).min(self.hero.max_hp);
                self.events.push(Event::Log(format!("You catch your breath by the stairwell (+{} hp).", rec)));
                self.events.push(Event::Log(format!("You descend to floor {}.", self.floor_no + 1)));
                return (r, self.check_done());
            }
            _ => {}
        }
        // monster phase
        let hp0 = self.hero.hp;
        self.monster_turn();
        let lost = (hp0 - self.hero.hp).max(0);
        if lost > 0 {
            r += R_HIT_TAKEN * (lost as f32 / 4.0).clamp(0.5, 3.0);
        }
        // secret door drafts: small chance adjacent secret opens
        if !self.dead {
            'outer: for oy in -1..=1 {
                for ox in -1..=1 {
                    if let Some(c) = self.map.at(self.hero.x + ox, self.hero.y + oy) {
                        if c.t == Tile::SecretDoor && self.rng.chance(0.06) {
                            let (sx, sy) = (self.hero.x + ox, self.hero.y + oy);
                            self.map.set(sx as usize, sy as usize).t = Tile::Door;
                            self.map.set(sx as usize, sy as usize).locked = -1;
                            self.events.push(Event::Log("A draft reveals a secret passage!".into()));
                            self.events.push(Event::Part { kind: 4, x: sx, y: sy });
                            break 'outer;
                        }
                    }
                }
            }
        }
        self.update_fov();
        (r, self.check_done())
    }

    fn check_done(&mut self) -> bool {
        if self.hero.hp <= 0 && !self.dead {
            self.dead = true;
            self.events.push(Event::Die);
            self.events.push(Event::Log(format!("You die on floor {} at turn {}. The dungeon claims another soul.", self.floor_no + 1, self.turn)));
        }
        self.dead || self.won || self.turn >= MAX_TURNS
    }

    fn hero_attack(&mut self) -> f32 {
        let mut best: Option<(i32, usize)> = None;
        for a in 0..8 {
            let (dx, dy) = DIRS[a];
            if let Some(mi) = self.monster_at(self.hero.x + dx, self.hero.y + dy) {
                let m = &self.monsters[mi];
                if m.hp > 0 {
                    match best {
                        None => best = Some((m.hp, mi)),
                        Some((bh, _)) if m.hp < bh => best = Some((m.hp, mi)),
                        _ => {}
                    }
                }
            }
        }
        let mi = match best {
            None => return R_WASTE - 0.5,
            Some((_, mi)) => mi,
        };
        let (hit, d20) = attack_roll(&mut self.rng, self.hero.to_hit, self.monsters[mi].ac);
        let (mx, my) = (self.monsters[mi].x, self.monsters[mi].y);
        let name = self.monsters[mi].name.clone();
        self.events.push(Event::Roll { ax: self.hero.x, ay: self.hero.y, tx: mx, ty: my, d20, bonus: self.hero.to_hit, ac: self.monsters[mi].ac, hit, hero: true });
        if hit {
            let crit = d20 == 20;
            let mut dmg = roll(self.hero.dmg.0, self.hero.dmg.1, &mut self.rng);
            if crit {
                dmg = (self.hero.dmg.0 * self.hero.dmg.1) as i32;
                self.events.push(Event::Shake(6.0));
            } else {
                self.events.push(Event::Shake(1.5));
            }
            self.monsters[mi].hp -= dmg;
            self.events.push(Event::Damage { x: mx, y: my, amount: dmg, crit, heal: false });
            self.events.push(Event::Part { kind: if self.monsters[mi].kind == Kind::SlimeBig || self.monsters[mi].kind == Kind::SlimeMini { 2 } else { 1 }, x: mx, y: my });
            self.events.push(Event::Part { kind: 0, x: mx, y: my });
            self.events.push(Event::Log(format!(
                "You roll {}+{} vs AC {}: hit {} for {}!",
                d20, self.hero.to_hit, self.monsters[mi].ac, name, dmg
            )));
            if self.monsters[mi].hp <= 0 {
                return self.monster_died(mi);
            } else if self.monsters[mi].kind == Kind::Lich {
                self.lich_teleport(mi);
            }
        } else {
            self.events.push(Event::Log(format!(
                "You roll {}+{} vs AC {}: miss {}",
                d20, self.hero.to_hit, self.monsters[mi].ac, name
            )));
        }
        0.0
    }

    fn monster_died(&mut self, mi: usize) -> f32 {
        let m = &self.monsters[mi];
        let (mx, my) = (m.x, m.y);
        let xp = m.xp;
        let kind = m.kind;
        let was_boss = m.is_boss;
        let name = m.name.clone();
        self.monsters[mi].hp = 0;
        let mut r = R_KILL_BASE + (xp as f32 / 10.0).min(12.0);
        self.events.push(Event::Part { kind: 1, x: mx, y: my });
        self.events.push(Event::Log(format!("{} dies! +{} xp", name, xp)));
        if self.hero.gain_xp(xp) {
            self.events.push(Event::LevelUp);
            self.events.push(Event::Log(format!("LEVEL UP! You are now level {}.", self.hero.level)));
        }
        if was_boss {
            r += 30.0;
            self.complete_quest(&Quest::SlayBoss);
        }
        if !self.elite_alive && !self.monsters.iter().any(|e| e.is_boss && e.hp > 0) {
            self.elite_alive = false;
        }
        match kind {
            Kind::SlimeBig => {
                // split into two slimelets
                let mut n = 0;
                for (dx, dy) in [(-1, -1), (1, 1), (-1, 1), (1, -1)] {
                    if n >= 2 {
                        break;
                    }
                    let (nx, ny) = (mx + dx, my + dy);
                    if self.map.passable(nx, ny)
                        && self.monster_at(nx, ny).is_none()
                        && !(nx == self.hero.x && ny == self.hero.y)
                    {
                        let mut s = Kind::SlimeMini.spawn(nx, ny, self.floor_no, &mut self.rng);
                        s.name = "Slimelet".into();
                        self.monsters.push(s);
                        n += 1;
                    }
                }
                if n > 0 {
                    self.events.push(Event::Log("The ooze splits!".into()));
                }
            }
            Kind::Dragon => {
                self.events.push(Event::Log("The ancient dragon crashes down!".into()));
            }
            _ => {}
        }
        r
    }

    fn pickup(&mut self) -> f32 {
        // pick nearest item within Chebyshev 1
        let mut bi: Option<usize> = None;
        let mut bd = i32::MAX;
        for (i, (ix, iy, _)) in self.items.iter().enumerate() {
            let d = (*ix as i32 - self.hero.x).abs().max(*iy as i32 - self.hero.y).abs();
            if d <= 1 && d < bd {
                bd = d;
                bi = Some(i);
            }
        }
        let Some(i) = bi else { return R_WASTE - 0.5 };
        let (ix, iy, kind) = self.items.remove(i);
        let mut r = R_PICKUP;
        match kind {
            ItemKind::Potion => {
                self.hero.potions += 1;
                self.events.push(Event::Log("Picked up a healing potion.".into()));
            }
            ItemKind::Gold => {
                let g = roll(2, 10, &mut self.rng) as u32 * (self.floor_no as u32 + 1);
                self.hero.gold += g;
                self.events.push(Event::Part { kind: 5, x: ix as i32, y: iy as i32 });
                self.events.push(Event::Log(format!("Picked up {} gold.", g)));
            }
            ItemKind::Key(t) => {
                self.hero.keys[t] += 1;
                r = R_KEY;
                self.events.push(Event::Part { kind: 3, x: ix as i32, y: iy as i32 });
                self.events.push(Event::Log(format!("Picked up a {} key!", TIER_NAMES[t])));
                if t == 2 && self.complete_quest(&Quest::FetchKey) {
                    r += R_QUEST;
                }
            }
        }
        r
    }

    fn open(&mut self) -> f32 {
        // nearest openable within Chebyshev 1
        let mut found: Option<(i32, i32, u8)> = None;
        let mut bd = i32::MAX;
        for oy in -1..=1 {
            for ox in -1..=1 {
                let (nx, ny) = (self.hero.x + ox, self.hero.y + oy);
                if let Some(c) = self.map.at(nx, ny) {
                    let i = self.map.idx(nx as usize, ny as usize);
                    let d = ox.abs().max(oy.abs());
                    let usable = match c.t {
                        Tile::Door | Tile::Vault => c.locked >= 0 && d < bd,
                        Tile::Chest => !self.opened_chests.contains(&i) && d < bd,
                        Tile::Fountain => !self.used_fountains.contains(&i) && ox == 0 && oy == 0,
                        _ => false,
                    };
                    if usable {
                        bd = d;
                        let code = match c.t {
                            Tile::Door | Tile::Vault => 0u8,
                            Tile::Chest => 1,
                            _ => 2,
                        };
                        found = Some((nx, ny, code));
                    }
                }
            }
        }
        let Some((x, y, code)) = found else { return R_WASTE - 0.5 };
        let i = self.map.idx(x as usize, y as usize);
        match code {
            0 => {
                let was_vault = self.map.cells[i].is_vault;
                let tier = self.map.cells[i].locked.max(0) as usize;
                if self.map.cells[i].locked >= 0 {
                    if !self.hero.has_key(tier) {
                        return R_WASTE - 0.5;
                    }
                    self.hero.keys[tier] -= 1; // keys are consumed (vault shares red key: guaranteed reachable per construction)
                    self.map.set(x as usize, y as usize).locked = -1;
                }
                self.map.set(x as usize, y as usize).t = Tile::Door;
                self.events.push(Event::Log(format!(
                    "You unlock the {} door.",
                    if was_vault { "vault" } else { TIER_NAMES[tier.min(2)] }
                )));
                self.events.push(Event::Part { kind: 4, x, y });
                R_DOOR
            }
            1 => {
                self.opened_chests.insert(i);
                self.events.push(Event::Part { kind: 4, x, y });
                let is_vault_chest = self.quests.vault_spot == Some((x as usize, y as usize));
                // mimic in chest?
                if let Some(mi) = self.monster_at(x, y) {
                    if self.monsters[mi].kind == Kind::Mimic {
                        self.events.push(Event::Log(format!("The chest is a {}! It attacks!", self.monsters[mi].name)));
                        let (hit, _) = attack_roll(&mut self.rng, self.monsters[mi].to_hit + 2, self.hero.ac);
                        if hit {
                            let dmg = roll(self.monsters[mi].dmg.0, self.monsters[mi].dmg.1, &mut self.rng);
                            self.hero.hp -= dmg;
                            self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: dmg, crit: false, heal: false });
                        }
                        self.map.set(x as usize, y as usize).t = Tile::Floor;
                        return R_WASTE;
                    }
                }
                let mut r = R_PICKUP;
                if is_vault_chest {
                    r += 40.0;
                    self.complete_quest(&Quest::OpenVault);
                    self.hero.gold += 500;
                    self.events.push(Event::Log("The vault bursts open: 500 gold and glory!".into()));
                    self.events.push(Event::Part { kind: 5, x, y });
                    self.events.push(Event::Shake(8.0));
                } else {
                    let rollv = self.rng.next_f32();
                    if rollv < 0.4 {
                        self.hero.potions += 1;
                        self.events.push(Event::Log("Chest: a healing potion.".into()));
                    } else if rollv < 0.55 {
                        let g = roll(3, 8, &mut self.rng) as u32 * (self.floor_no as u32 + 2);
                        self.hero.gold += g;
                        self.events.push(Event::Log(format!("Chest: {} gold.", g)));
                    } else if rollv < 0.75 {
                        self.hero.dmg = (self.hero.dmg.0 + 1, self.hero.dmg.1);
                        self.events.push(Event::Log(format!("Chest: a keener blade! Damage now {}d{}.", self.hero.dmg.0, self.hero.dmg.1)));
                    } else if rollv < 0.9 {
                        self.hero.ac += 1;
                        self.events.push(Event::Log(format!("Chest: better armor! AC now {}.", self.hero.ac)));
                    } else {
                        let t = self.floor_no.min(2);
                        self.hero.keys[t] += 1;
                        self.events.push(Event::Log(format!("Chest: a {} key!", TIER_NAMES[t])));
                    }
                }
                r
            }
            _ => {
                self.used_fountains.insert(i);
                let h = self.hero.max_hp - self.hero.hp;
                self.hero.hp = self.hero.max_hp;
                self.events.push(Event::Log(format!("The fountain heals you (+{} hp).", h)));
                R_PICKUP
            }
        }
    }

    fn complete_quest(&mut self, q: &Quest) -> bool {
        let mut done_now = false;
        for (qq, d) in self.quests.quests.iter_mut() {
            if qq == q && !*d {
                *d = true;
                done_now = true;
                self.events.push(Event::Log(format!("QUEST COMPLETE: {}", q.label())));
            }
        }
        if *q == Quest::FetchKey && done_now {
            self.quests.key_found = true;
        }
        done_now
    }

    pub fn deepest_floor(&self) -> usize {
        self.floor_no + 1
    }
}

include!("sim_ai.rs");
