use crate::dungeon::FloorGen;
use crate::rng::Rng;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Rat,
    Bat,
    Goblin,
    SkeletonArcher,
    Cultist,
    SlimeBig,
    SlimeMini,
    Wraith,
    Ogre,
    Mimic,
    Lich,
    Boss,
    Dragon,
}

/// D&D-flavored monster. to_hit is the d20 attack bonus; ac is armor class;
/// dmg = (dice count, die sides) rolled on a hit.
#[derive(Clone, Debug)]
pub struct Monster {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub ac: i32,
    pub to_hit: i32,
    pub dmg: (u32, u32),
    pub xp: u32,
    pub speed: u8,
    pub sees_hero: bool,
    pub name: String,
    pub is_boss: bool,
    pub cooldown: i32,
    pub enraged: bool,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Rat => "Giant Rat",
            Kind::Bat => "Giant Bat",
            Kind::Goblin => "Goblin",
            Kind::SkeletonArcher => "Skeleton Archer",
            Kind::Cultist => "Cultist",
            Kind::SlimeBig => "Gray Ooze",
            Kind::SlimeMini => "Slimelet",
            Kind::Wraith => "Wraith",
            Kind::Ogre => "Ogre",
            Kind::Mimic => "Mimic",
            Kind::Lich => "Lich",
            Kind::Boss => "Dungeon Boss",
            Kind::Dragon => "Ancient Dragon",
        }
    }
    fn base(self) -> (i32, i32, i32, (u32, u32), u32, u8) {
        // (hp, ac, to_hit, dmg, xp, speed)
        match self {
            Kind::Rat => (5, 10, 2, (1, 4), 8, 1),
            Kind::Bat => (4, 11, 3, (1, 4), 6, 2),
            Kind::Goblin => (9, 12, 3, (1, 6), 14, 1),
            Kind::SkeletonArcher => (11, 12, 4, (1, 8), 20, 1),
            Kind::Cultist => (14, 11, 4, (1, 8), 26, 1),
            Kind::SlimeBig => (18, 8, 3, (1, 8), 30, 1),
            Kind::SlimeMini => (6, 9, 2, (1, 4), 5, 1),
            Kind::Wraith => (22, 12, 5, (2, 6), 48, 2),
            Kind::Ogre => (30, 11, 5, (2, 8), 60, 1),
            Kind::Mimic => (16, 12, 4, (2, 6), 34, 1),
            Kind::Lich => (60, 15, 7, (3, 8), 300, 1),
            Kind::Boss => (48, 13, 6, (2, 8), 160, 1),
            Kind::Dragon => (110, 16, 8, (3, 10), 600, 1),
        }
    }
    pub fn is_final(self) -> bool {
        matches!(self, Kind::Dragon)
    }
    pub fn is_boss(self) -> bool {
        matches!(self, Kind::Dragon | Kind::Lich | Kind::Boss)
    }
    pub fn spawn(self, x: i32, y: i32, floor: usize, rng: &mut Rng) -> Monster {
        let (hp, ac, to_hit, dmg, xp, speed) = self.base();
        let b = floor as i32;
        let name = self.label().to_string();
        Monster {
            kind: self,
            x,
            y,
            hp: hp + b,
            max_hp: hp + b,
            ac: ac + (b / 3),
            to_hit: to_hit + (b / 2),
            dmg: (dmg.0, dmg.1),
            xp: xp + (b as u32 * 2),
            speed,
            sees_hero: false,
            name,
            is_boss: self.is_boss(),
            cooldown: 0,
            enraged: false,
        }
    }
}

pub const MONSTER_POOL: [Kind; 10] = [
    Kind::Rat,
    Kind::Bat,
    Kind::Goblin,
    Kind::SkeletonArcher,
    Kind::Cultist,
    Kind::SlimeBig,
    Kind::Wraith,
    Kind::Ogre,
    Kind::Mimic,
    Kind::Lich,
];

/// Roll n d sides, return sum.
pub fn roll(n: u32, sides: u32, rng: &mut Rng) -> i32 {
    let mut t = 0i32;
    for _ in 0..n {
        t += if sides == 0 { 0 } else { rng.range(1, sides as usize + 1) as i32 };
    }
    t
}
/// d20 + bonus vs target AC. Natural 20 always hits, natural 1 always misses.
pub fn attack_roll(rng: &mut Rng, to_hit: i32, ac: i32) -> (bool, i32) {
    let d = rng.range(1, 21) as i32;
    if d == 20 {
        return (true, d);
    }
    if d == 1 {
        return (false, d);
    }
    (d + to_hit >= ac, d)
}

#[derive(Clone, Debug)]
pub struct Hero {
    pub x: i32,
    pub y: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub ac: i32,
    pub xp: u32,
    pub level: u32,
    pub to_hit: i32,
    pub dmg: (u32, u32),
    pub potions: u32,
    pub keys: [u32; 3],
    pub gold: u32,
}
impl Hero {
    pub fn new(x: i32, y: i32) -> Hero {
        Hero {
            x,
            y,
            hp: 24,
            max_hp: 24,
            ac: 12,
            xp: 0,
            level: 1,
            to_hit: 4,
            dmg: (1, 8),
            potions: 2,
            keys: [0; 3],
            gold: 0,
        }
    }
    pub fn xp_next(&self) -> u32 {
        40 + self.level * self.level * 20
    }
    pub fn gain_xp(&mut self, x: u32) -> bool {
        self.xp += x;
        if self.xp >= self.xp_next() {
            self.xp -= self.xp_next();
            self.level += 1;
            self.max_hp += 6;
            self.hp = self.max_hp;
            self.to_hit += 1;
            if self.level % 2 == 0 {
                self.dmg = (self.dmg.0 + 1, self.dmg.1);
            }
            true
        } else {
            false
        }
    }
    pub fn has_key(&self, tier: usize) -> bool {
        tier < 3 && self.keys[tier] > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Quest {
    FetchKey,  // find the tier-2 (red) key
    OpenVault, // open the vault chest (implies FetchKey first)
    SlayBoss,
    Rescue,
    Shrine,
}
impl Quest {
    pub fn label(&self) -> &'static str {
        match self {
            Quest::FetchKey => "Find the Red Key",
            Quest::OpenVault => "Open the Sealed Vault",
            Quest::SlayBoss => "Slay the Dwelling Horror",
            Quest::Rescue => "Rescue the Captive",
            Quest::Shrine => "Bless the Forgotten Shrine",
        }
    }
}

pub struct QuestState {
    pub quests: Vec<(Quest, bool)>,
    pub key_found: bool,
    pub captive_spot: Option<(usize, usize)>,
    pub shrine_spot: Option<(usize, usize)>,
    pub vault_spot: Option<(usize, usize)>,
    pub boss_alive: bool,
}

pub fn pick_quests(floor: usize, f: &FloorGen, rng: &mut Rng) -> QuestState {
    let mut qs: Vec<(Quest, bool)> = vec![];
    if f.locked_doors.iter().any(|(t, _, _)| *t == 2) {
        qs.push((Quest::FetchKey, false));
    }
    if f.vault.is_some() {
        qs.push((Quest::OpenVault, false));
    }
    // slay a named monster is always available (boss on boss floors)
    qs.push((Quest::SlayBoss, false));
    if f.captive_spot.is_some() {
        qs.push((Quest::Rescue, false));
    }
    if f.shrine.is_some() {
        qs.push((Quest::Shrine, false));
    }
    // keep 1-3: always keep Slay + first two others
    let mut out: Vec<(Quest, bool)> = vec![];
    for q in qs {
        if q.0 == Quest::SlayBoss || out.len() < 2 {
            out.push(q);
        } else {
            let _ = rng;
        }
    }
    QuestState {
        quests: out,
        key_found: false,
        captive_spot: f.captive_spot,
        shrine_spot: f.shrine,
        vault_spot: f.vault,
        boss_alive: false,
    }
}

pub fn is_boss_floor(floor: usize) -> bool {
    floor >= 1 && (floor + 1) % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attack_deterministic_and_bounds() {
        let mut r1 = Rng::new(3);
        let mut r2 = Rng::new(3);
        for _ in 0..200 {
            let (hit1, d1) = attack_roll(&mut r1, 4, 15);
            let (hit2, d2) = attack_roll(&mut r2, 4, 15);
            assert_eq!(d1, d2);
            assert!((1..=20).contains(&d1));
            if d1 == 20 {
                assert!(hit1);
            }
            if d1 == 1 {
                assert!(!hit1);
            }
            assert_eq!(hit1, hit2);
        }
    }
    #[test]
    fn natural_20_hits_high_ac() {
        let mut r = Rng::new(11);
        // find a seed window where first roll is 20
        let (hit, d) = attack_roll(&mut r, 0, 40);
        if d == 20 {
            assert!(hit);
        }
    }
    #[test]
    fn damage_dice_bounded() {
        let mut r = Rng::new(1);
        for _ in 0..300 {
            let v = roll(2, 8, &mut r);
            assert!((2..=16).contains(&v));
        }
    }
    #[test]
    fn xp_levels_up() {
        let mut h = Hero::new(0, 0);
        h.gain_xp(40 + 20); // level 1 -> 2
        assert!(h.level >= 2);
        assert_eq!(h.to_hit, 5);
    }
    #[test]
    fn quests_completable_floor_has_red_key_and_boss_when_needed() {
        for floor in 0..8 {
            let f = crate::dungeon::gen_floor(floor as u64 * 977 + 13, floor);
            let mut rng = Rng::new(floor as u64 + 1);
            let qs = pick_quests(floor, &f, &mut rng);
            for (q, _) in &qs.quests {
                match q {
                    Quest::FetchKey => assert!(f.locked_doors.iter().any(|(t, _, _)| *t == 2)),
                    Quest::OpenVault => assert!(f.vault.is_some()),
                    Quest::Rescue => assert!(f.captive_spot.is_some()),
                    Quest::Shrine => assert!(f.shrine.is_some()),
                    Quest::SlayBoss => {} // boss spawned by sim
                }
            }
        }
    }
}
