// part 4 of sim: monster AI. (included by sim_act.rs)
use super::*;

impl World {
    pub fn monster_turn(&mut self) {
        if self.dead || self.won {
            return;
        }
        let n = self.monsters.len();
        let mut dmg_to_hero = 0i32;
        for mi in 0..n {
            if self.monsters[mi].hp <= 0 {
                continue;
            }
            let (mx, my) = (self.monsters[mi].x, self.monsters[mi].y);
            let (dx, dy) = (self.hero.x - mx, self.hero.y - my);
            let cheb = dx.abs().max(dy.abs());
            let kind = self.monsters[mi].kind;
            let speed = self.monsters[mi].speed as i32;
            let sees = self.in_sight(mi);
            self.monsters[mi].sees_hero = sees;
            if self.monsters[mi].cooldown > 0 {
                self.monsters[mi].cooldown -= 1;
            }
            // ranged specials first
            match kind {
                Kind::SkeletonArcher if sees && cheb >= 3 && cheb <= 7 && self.monsters[mi].cooldown == 0 => {
                    self.monsters[mi].cooldown = 3;
                    let (hit, d20) = attack_roll(&mut self.rng, self.monsters[mi].to_hit, self.hero.ac);
                    let nm = self.monsters[mi].name.clone();
                    self.events.push(Event::Roll { ax: self.monsters[mi].x, ay: self.monsters[mi].y, tx: self.hero.x, ty: self.hero.y, d20, bonus: self.monsters[mi].to_hit, ac: self.hero.ac, hit, hero: false });
                    if hit {
                        let d = roll(self.monsters[mi].dmg.0, self.monsters[mi].dmg.1, &mut self.rng);
                        dmg_to_hero += d;
                        self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: d, crit: false, heal: false });
                        self.events.push(Event::Log(format!("{} fires an arrow: {} dmg!", nm, d)));
                    } else {
                        self.events.push(Event::Log(format!("{} fires an arrow — it clatters past you.", nm)));
                    }
                    continue;
                }
                Kind::Cultist if sees && cheb >= 2 && cheb <= 5 && self.monsters[mi].cooldown == 0 => {
                    self.monsters[mi].cooldown = 4;
                    let (hit, _) = attack_roll(&mut self.rng, self.monsters[mi].to_hit, self.hero.ac);
                    let nm = self.monsters[mi].name.clone();
                    self.events.push(Event::Part { kind: 3, x: self.hero.x, y: self.hero.y });
                    if hit {
                        let d = roll(self.monsters[mi].dmg.0, self.monsters[mi].dmg.1, &mut self.rng);
                        dmg_to_hero += d;
                        self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: d, crit: false, heal: false });
                        self.events.push(Event::Log(format!("{} hurls eldritch fire: {} dmg!", nm, d)));
                    } else {
                        self.events.push(Event::Log(format!("{}'s eldritch fire fizzles.", nm)));
                    }
                    continue;
                }
                Kind::Dragon if sees && cheb <= 6 && self.monsters[mi].cooldown == 0 => {
                    // fire breath cone-ish: line to hero, damage tiles near line
                    self.monsters[mi].cooldown = 5;
                    let nm = self.monsters[mi].name.clone();
                    self.events.push(Event::Shake(10.0));
                    for step in 0..=cheb {
                        let t = step as f32 / cheb.max(1) as f32;
                        let bx = mx + (dx as f32 * t).round() as i32;
                        let by = my + (dy as f32 * t).round() as i32;
                        self.events.push(Event::Part { kind: 3, x: bx, y: by });
                    }
                    let (hit, _) = attack_roll(&mut self.rng, self.monsters[mi].to_hit, self.hero.ac);
                    if hit {
                        let d = roll(4, 8, &mut self.rng);
                        dmg_to_hero += d;
                        self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: d, crit: true, heal: false });
                        self.events.push(Event::Log(format!("{} breathes FIRE: {} dmg!", nm, d)));
                    } else {
                        self.events.push(Event::Log(format!("{}'s fire washes over you — you dive clear!", nm)));
                    }
                    continue;
                }
                Kind::Lich => {
                    if sees && cheb <= 5 && self.monsters[mi].cooldown == 0 {
                        self.monsters[mi].cooldown = 5;
                        let (hit, _) = attack_roll(&mut self.rng, self.monsters[mi].to_hit, self.hero.ac);
                        let nm = self.monsters[mi].name.clone();
                        if hit {
                            let d = roll(self.monsters[mi].dmg.0, self.monsters[mi].dmg.1, &mut self.rng);
                            dmg_to_hero += d;
                            self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: d, crit: false, heal: false });
                            self.events.push(Event::Log(format!("{} casts drain life: {} dmg!", nm, d)));
                        }
                        // chance to raise a servant
                        if self.rng.chance(0.4) && self.monsters.len() < 40 {
                            let mut lx = mx;
                            let mut ly = my;
                            for _ in 0..40 {
                                let (nx, ny) = (mx + self.rng.range(0, 7) as i32 - 3, my + self.rng.range(0, 7) as i32 - 3);
                                if self.map.passable(nx, ny) && self.monster_at(nx, ny).is_none() {
                                    lx = nx;
                                    ly = ny;
                                    break;
                                }
                            }
                            let mut b = Kind::Bat.spawn(lx, ly, self.floor_no, &mut self.rng);
                            b.name = "Bone Servant".into();
                            self.monsters.push(b);
                            self.events.push(Event::Log("The lich raises a bone servant!".into()));
                        }
                        continue;
                    }
                }
                _ => {}
            }
            // melee
            if cheb <= 1 {
                if sees || cheb == 0 {
                    let (hit, d20) = {
                        let to_hit = self.monsters[mi].to_hit;
                        let ac = self.hero.ac;
                        attack_roll(&mut self.rng, to_hit, ac)
                    };
                    let nm = self.monsters[mi].name.clone();
                    self.events.push(Event::Roll { ax: self.monsters[mi].x, ay: self.monsters[mi].y, tx: self.hero.x, ty: self.hero.y, d20, bonus: self.monsters[mi].to_hit, ac: self.hero.ac, hit, hero: false });
                    if hit {
                        let (dn, ds) = self.monsters[mi].dmg;
                        let d = roll(dn, ds, &mut self.rng);
                        dmg_to_hero += d;
                        let crit = d20 == 20;
                        self.events.push(Event::Damage { x: self.hero.x, y: self.hero.y, amount: d, crit, heal: false });
                        self.events.push(Event::Part { kind: 1, x: self.hero.x, y: self.hero.y });
                        self.events.push(Event::Shake(if crit { 7.0 } else { 2.5 }));
                        self.events.push(Event::Log(format!("{} hits you for {}.", nm, d)));
                    } else {
                        self.events.push(Event::Log(format!("{} misses.", nm)));
                    }
                }
                continue;
            }
            // movement toward hero when aggro
            if !sees {
                continue;
            }
            let pass_wall = kind == Kind::Wraith;
            for _ in 0..speed {
                let (cx0, cy0) = (self.monsters[mi].x, self.monsters[mi].y);
                let (dx, dy) = (self.hero.x - cx0, self.hero.y - cy0);
                if dx.abs().max(dy.abs()) <= 1 {
                    break;
                }
                let mut order: Vec<(i32, i32)> = vec![];
                let sx = dx.signum();
                let sy = dy.signum();
                if dx.abs() >= dy.abs() {
                    order.push((sx, 0));
                    if sy != 0 {
                        order.push((sx, sy));
                        order.push((0, sy));
                    }
                } else {
                    order.push((0, sy));
                    if sx != 0 {
                        order.push((sx, sy));
                        order.push((sx, 0));
                    }
                }
                let mut moved = false;
                for (ox, oy) in order {
                    let (nx, ny) = (cx0 + ox, cy0 + oy);
                    if nx == self.hero.x && ny == self.hero.y {
                        continue;
                    }
                    let ok = if pass_wall {
                        nx >= 0 && ny >= 0 && (nx as usize) < self.map.w && (ny as usize) < self.map.h
                    } else {
                        self.map.passable(nx, ny)
                            && self.monsters.iter().enumerate().all(|(j, o)| j == mi || o.hp <= 0 || (o.x, o.y) != (nx, ny))
                    };
                    if ok {
                        self.monsters[mi].x = nx;
                        self.monsters[mi].y = ny;
                        moved = true;
                        break;
                    }
                }
                if !moved {
                    break;
                }
            }
        }
        if dmg_to_hero > 0 {
            self.hero.hp -= dmg_to_hero;
        }
        // captive rescue: standing adjacent to captive spot with quests pending kills guard? rescue = reach spot
        if let Some((cxp, cyp)) = self.quests.captive_spot {
            if (cxp as i32 - self.hero.x).abs() <= 1 && (cyp as i32 - self.hero.y).abs() <= 1 {
                if self.complete_quest(&Quest::Rescue) {
                    self.hero.xp += 30;
                }
            }
        }
    }

    fn in_sight(&self, mi: usize) -> bool {
        let m = &self.monsters[mi];
        let (dx, dy) = (m.x - self.hero.x, m.y - self.hero.y);
        let d = dx.abs().max(dy.abs());
        let range = if m.kind == Kind::Wraith { 8 } else { 10 };
        if d > range {
            return false;
        }
        if m.kind == Kind::Wraith {
            return true;
        }
        fov::has_los(m.x, m.y, self.hero.x, self.hero.y, &|x, y| self.solid_for_sight(x, y))
    }

    fn lich_teleport(&mut self, mi: usize) {
        if !self.rng.chance(0.35) {
            return;
        }
        let (ox, oy) = (self.monsters[mi].x, self.monsters[mi].y);
        for _ in 0..40 {
            let nx = ox + self.rng.range(0, 17) as i32 - 8;
            let ny = oy + self.rng.range(0, 17) as i32 - 8;
            if self.map.passable(nx, ny)
                && self.monster_at(nx, ny).is_none()
                && (nx - self.hero.x).abs().max((ny - self.hero.y).abs()) >= 4
            {
                self.monsters[mi].x = nx;
                self.monsters[mi].y = ny;
                self.events.push(Event::Part { kind: 3, x: ox, y: oy });
                self.events.push(Event::Part { kind: 3, x: nx, y: ny });
                self.events.push(Event::Log(format!("{} blinks away!", self.monsters[mi].name)));
                break;
            }
        }
    }
}
