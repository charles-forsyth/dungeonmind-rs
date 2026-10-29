// Procedural pixel art for DungeonMind. All 16x16, generated at startup.
use dungeonmind::entities::Kind;
use dungeonmind::rng::Rng;
use macroquad::prelude::*;

pub const TS: u32 = 16; // texture size

struct Img {
    d: Vec<Color>,
}
impl Img {
    fn new() -> Img {
        Img {
            d: vec![Color::new(0.0, 0.0, 0.0, 0.0); (TS * TS) as usize],
        }
    }
    fn px(&mut self, x: u32, y: u32, c: Color) {
        if x < TS && y < TS {
            self.d[(y * TS + x) as usize] = c;
        }
    }
    fn rect(&mut self, x0: u32, y0: u32, w: u32, h: u32, c: Color) {
        for y in y0..(y0 + h).min(TS) {
            for x in x0..(x0 + w).min(TS) {
                self.px(x, y, c);
            }
        }
    }
    fn disc(&mut self, cx: i32, cy: i32, r: i32, c: Color) {
        for y in 0..TS as i32 {
            for x in 0..TS as i32 {
                let dx = x - cx;
                let dy = y - cy;
                if dx * dx + dy * dy <= r * r {
                    self.px(x as u32, y as u32, c);
                }
            }
        }
    }
    fn noise(&mut self, rng: &mut Rng, base: Color, amt: f32) {
        for y in 0..TS {
            for x in 0..TS {
                let n = (rng.next_f32() - 0.5) * amt;
                self.px(
                    x,
                    y,
                    Color::new(
                        (base.r + n).clamp(0.0, 1.0),
                        (base.g + n).clamp(0.0, 1.0),
                        (base.b + n).clamp(0.0, 1.0),
                        1.0,
                    ),
                );
            }
        }
    }
    fn tex(self) -> Texture2D {
        let mut img = Image::gen_image_color(TS as u16, TS as u16, Color::new(0.0, 0.0, 0.0, 0.0));
        for y in 0..TS {
            for x in 0..TS {
                img.set_pixel(x, y, self.d[(y * TS + x) as usize]);
            }
        }
        let t = Texture2D::from_image(&img);
        t.set_filter(FilterMode::Nearest);
        t
    }
}

pub const AMBER: Color = Color::new(0.83, 0.69, 0.22, 1.0);
pub const PARCH: Color = Color::new(0.9, 0.86, 0.75, 1.0);
pub const CWHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
pub const TIER_COLORS: [Color; 3] = [
    Color::new(0.25, 0.45, 0.95, 1.0),
    Color::new(0.25, 0.8, 0.35, 1.0),
    Color::new(0.9, 0.25, 0.25, 1.0),
];

pub struct Art {
    pub wall: Texture2D,
    pub floor: Texture2D,
    pub corridor: Texture2D,
    pub cave: Texture2D,
    pub water: Texture2D,
    pub lava: Texture2D,
    pub doors: [Texture2D; 3], // locked tiers
    pub door_open: Texture2D,
    pub door_plain: Texture2D,
    pub vault: Texture2D,
    pub chest: Texture2D,
    pub chest_open: Texture2D,
    pub fountain: Texture2D,
    pub fountain_used: Texture2D,
    pub trap: Texture2D,
    pub stairs: Texture2D,
    pub shrine: Texture2D,
    pub captive: Texture2D,
    pub hero: Texture2D,
    pub mons: Vec<(Kind, Texture2D)>,
    pub torch_off: Texture2D,
}

impl Art {
    pub fn mon_tex(&self, k: Kind) -> &Texture2D {
        self.mons.iter().find(|(kk, _)| *kk == k).map(|(_, t)| t).unwrap_or(&self.mons[0].1)
    }
}

pub fn gen_art() -> Art {
    let mut rng = Rng::new(0xA815);
    // ---- terrain ----
    let mut wall = Img::new();
    wall.noise(&mut rng, Color::new(0.30, 0.27, 0.36, 1.0), 0.07);
    // brick seams
    for y in [0u32, 4, 8, 12] {
        wall.rect(0, y, TS, 1, Color::new(0.18, 0.16, 0.22, 1.0));
    }
    for y in 0..4 {
        wall.px(((y % 2) * 8 + 4) % TS, y, Color::new(0.11, 0.1, 0.14, 1.0));
        wall.px(((y % 2) * 8 + 4 + 8) % TS, y, Color::new(0.11, 0.1, 0.14, 1.0));
    }
    let mut floor = Img::new();
    floor.noise(&mut rng, Color::new(0.24, 0.22, 0.29, 1.0), 0.05);
    floor.rect(0, 7, TS, 1, Color::new(0.17, 0.16, 0.21, 1.0));
    floor.rect(7, 0, 1, TS, Color::new(0.17, 0.16, 0.21, 1.0));
    let mut corridor = Img::new();
    corridor.noise(&mut rng, Color::new(0.27, 0.25, 0.32, 1.0), 0.05);
    let mut cave = Img::new();
    cave.noise(&mut rng, Color::new(0.26, 0.26, 0.23, 1.0), 0.08);
    for _ in 0..10 {
        let x = rng.range(0, TS as usize) as u32;
        let y = rng.range(0, TS as usize) as u32;
        cave.rect(x, y, 2, 1, Color::new(0.2, 0.19, 0.16, 1.0));
    }
    let mut water = Img::new();
    water.noise(&mut rng, Color::new(0.1, 0.22, 0.45, 1.0), 0.06);
    for y in [2u32, 6, 10, 14] {
        water.rect(0, y, TS, 1, Color::new(0.16, 0.32, 0.6, 1.0));
    }
    let mut lava = Img::new();
    lava.noise(&mut rng, Color::new(0.75, 0.25, 0.05, 1.0), 0.12);
    for _ in 0..6 {
        let x = rng.range(0, TS as usize) as u32;
        let y = rng.range(0, TS as usize) as u32;
        lava.px(x, y, Color::new(1.0, 0.8, 0.2, 1.0));
    }
    let mk_door = |rng: &mut Rng, lock: Option<Color>| {
        let mut d = Img::new();
        d.noise(rng, Color::new(0.35, 0.24, 0.14, 1.0), 0.05);
        d.rect(0, 0, 1, TS, Color::new(0.2, 0.13, 0.08, 1.0));
        d.rect(TS - 1, 0, 1, TS, Color::new(0.2, 0.13, 0.08, 1.0));
        d.rect(0, 0, TS, 1, Color::new(0.2, 0.13, 0.08, 1.0));
        d.rect(0, TS - 1, TS, 1, Color::new(0.2, 0.13, 0.08, 1.0));
        d.rect(6, 2, 4, 3, Color::new(0.25, 0.17, 0.1, 1.0));
        d.rect(6, 11, 4, 3, Color::new(0.25, 0.17, 0.1, 1.0));
        if let Some(c) = lock {
            d.rect(6, 7, 4, 3, c);
            d.px(8, 8, Color::new(0.05, 0.05, 0.05, 1.0));
        }
        d
    };
    let doors = [
        mk_door(&mut rng, Some(TIER_COLORS[0])).tex(),
        mk_door(&mut rng, Some(TIER_COLORS[1])).tex(),
        mk_door(&mut rng, Some(TIER_COLORS[2])).tex(),
    ];
    let door_plain = mk_door(&mut rng, None).tex();
    let mut door_open = Img::new();
    door_open.rect(0, 0, 2, TS, Color::new(0.25, 0.17, 0.1, 1.0));
    door_open.rect(TS - 2, 0, 2, TS, Color::new(0.25, 0.17, 0.1, 1.0));
    door_open.noise(&mut rng, Color::new(0.12, 0.11, 0.14, 1.0), 0.03);
    door_open.rect(0, 0, 2, TS, Color::new(0.25, 0.17, 0.1, 1.0));
    door_open.rect(TS - 2, 0, 2, TS, Color::new(0.25, 0.17, 0.1, 1.0));
    let mut vault = mk_door(&mut rng, Some(AMBER));
    vault.rect(3, 3, 10, 10, Color::new(0.45, 0.36, 0.12, 1.0));
    vault.rect(4, 4, 8, 8, Color::new(0.7, 0.57, 0.2, 1.0));
    vault.rect(7, 7, 2, 2, Color::new(0.3, 0.24, 0.08, 1.0));
    let mut chest = Img::new();
    chest.rect(2, 6, 12, 8, Color::new(0.42, 0.28, 0.14, 1.0));
    chest.rect(2, 4, 12, 3, Color::new(0.5, 0.34, 0.17, 1.0));
    chest.rect(7, 4, 2, 10, AMBER);
    chest.px(8, 9, Color::new(0.2, 0.16, 0.05, 1.0));
    let mut chest_open = Img::new();
    chest_open.rect(2, 8, 12, 6, Color::new(0.35, 0.23, 0.11, 1.0));
    chest_open.rect(2, 3, 12, 3, Color::new(0.5, 0.34, 0.17, 1.0));
    chest_open.rect(4, 6, 8, 3, Color::new(0.7, 0.57, 0.2, 1.0));
    let mut fountain = Img::new();
    fountain.disc(8, 9, 6, Color::new(0.3, 0.3, 0.36, 1.0));
    fountain.disc(8, 9, 5, Color::new(0.2, 0.55, 0.8, 1.0));
    fountain.disc(8, 9, 3, Color::new(0.4, 0.8, 1.0, 1.0));
    let mut fountain_used = Img::new();
    fountain_used.disc(8, 9, 6, Color::new(0.25, 0.25, 0.3, 1.0));
    fountain_used.disc(8, 9, 4, Color::new(0.15, 0.2, 0.25, 1.0));
    let mut trap = Img::new();
    trap.rect(3, 3, 10, 10, Color::new(0.1, 0.09, 0.12, 1.0));
    for i in 0..4 {
        let x = 4 + i * 3;
        trap.px(x, 6, Color::new(0.7, 0.7, 0.75, 1.0));
        trap.px(x, 8, Color::new(0.55, 0.55, 0.6, 1.0));
        trap.px(x, 10, Color::new(0.4, 0.4, 0.45, 1.0));
    }
    let mut stairs = Img::new();
    for i in 0..4 {
        stairs.rect(2, 2 + i * 4, 12 - i * 2, 3, Color::new(0.3 - i as f32 * 0.05, 0.28 - i as f32 * 0.05, 0.35 - i as f32 * 0.05, 1.0));
    }
    let mut shrine = Img::new();
    shrine.rect(5, 10, 6, 4, Color::new(0.35, 0.33, 0.45, 1.0));
    shrine.rect(6, 5, 4, 5, Color::new(0.45, 0.42, 0.6, 1.0));
    shrine.rect(7, 2, 2, 3, Color::new(0.75, 0.7, 1.0, 1.0));
    let mut captive = Img::new();
    captive.disc(8, 5, 2, PARCH);
    captive.rect(6, 7, 4, 6, Color::new(0.5, 0.4, 0.3, 1.0));
    captive.rect(4, 8, 2, 1, Color::new(0.7, 0.7, 0.75, 1.0));
    captive.rect(10, 8, 2, 1, Color::new(0.7, 0.7, 0.75, 1.0));
    let mut torch_off = Img::new();
    torch_off.noise(&mut rng, Color::new(0.22, 0.2, 0.26, 1.0), 0.05);
    torch_off.rect(7, 6, 2, 6, Color::new(0.3, 0.2, 0.12, 1.0));
    let mut hero = Img::new();
    hero.disc(8, 4, 2, PARCH); // helm
    hero.rect(7, 3, 3, 1, Color::new(0.75, 0.2, 0.2, 1.0)); // plume
    hero.rect(5, 7, 6, 6, Color::new(0.7, 0.68, 0.72, 1.0)); // mail
    hero.rect(4, 13, 2, 3, Color::new(0.3, 0.28, 0.35, 1.0));
    hero.rect(10, 13, 2, 3, Color::new(0.3, 0.28, 0.35, 1.0));
    hero.rect(12, 5, 1, 8, AMBER); // sword
    hero.px(11, 8, Color::new(0.5, 0.4, 0.2, 1.0));
    // ---- monsters ----
    let mut mons = Vec::new();
    {
        let mut i = Img::new();
        i.disc(8, 9, 4, Color::new(0.45, 0.42, 0.4, 1.0)); // rat
        i.rect(11, 8, 3, 2, Color::new(0.5, 0.47, 0.45, 1.0));
        i.px(13, 8, Color::new(0.9, 0.2, 0.2, 1.0));
        i.rect(3, 12, 1, 3, Color::new(0.35, 0.32, 0.3, 1.0));
        mons.push((Kind::Rat, i.tex()));
        let mut i = Img::new();
        i.disc(8, 8, 2, Color::new(0.35, 0.25, 0.45, 1.0)); // bat
        i.rect(1, 5, 6, 2, Color::new(0.3, 0.2, 0.4, 1.0));
        i.rect(9, 5, 6, 2, Color::new(0.3, 0.2, 0.4, 1.0));
        i.px(7, 7, Color::new(1.0, 0.9, 0.3, 1.0));
        i.px(9, 7, Color::new(1.0, 0.9, 0.3, 1.0));
        mons.push((Kind::Bat, i.tex()));
        let mut i = Img::new();
        i.disc(8, 6, 3, Color::new(0.3, 0.55, 0.25, 1.0)); // goblin
        i.rect(6, 9, 4, 5, Color::new(0.4, 0.32, 0.2, 1.0));
        i.px(6, 5, Color::new(0.9, 0.9, 0.3, 1.0));
        i.px(10, 5, Color::new(0.9, 0.9, 0.3, 1.0));
        i.rect(11, 8, 1, 5, Color::new(0.5, 0.5, 0.55, 1.0));
        mons.push((Kind::Goblin, i.tex()));
        let mut i = Img::new();
        i.disc(8, 4, 2, Color::new(0.85, 0.85, 0.8, 1.0)); // skeleton archer
        i.rect(7, 6, 2, 6, Color::new(0.8, 0.8, 0.75, 1.0));
        i.rect(9, 3, 1, 9, Color::new(0.45, 0.3, 0.15, 1.0));
        i.px(7, 4, Color::new(0.1, 0.1, 0.1, 1.0));
        i.px(9, 4, Color::new(0.1, 0.1, 0.1, 1.0));
        mons.push((Kind::SkeletonArcher, i.tex()));
        let mut i = Img::new();
        i.rect(5, 4, 6, 10, Color::new(0.55, 0.12, 0.15, 1.0)); // cultist robe
        i.disc(8, 4, 2, Color::new(0.3, 0.06, 0.08, 1.0));
        i.px(7, 4, AMBER);
        i.px(9, 4, AMBER);
        mons.push((Kind::Cultist, i.tex()));
        let mut i = Img::new();
        i.disc(8, 10, 5, Color::new(0.45, 0.45, 0.42, 0.9)); // gray ooze
        i.disc(6, 8, 2, Color::new(0.55, 0.55, 0.5, 1.0));
        i.px(6, 9, Color::new(0.1, 0.1, 0.1, 1.0));
        i.px(10, 9, Color::new(0.1, 0.1, 0.1, 1.0));
        mons.push((Kind::SlimeBig, i.tex()));
        let mut i = Img::new();
        i.disc(8, 10, 3, Color::new(0.5, 0.5, 0.45, 0.9)); // slimelet
        i.px(7, 9, Color::new(0.1, 0.1, 0.1, 1.0));
        mons.push((Kind::SlimeMini, i.tex()));
        let mut i = Img::new();
        i.rect(5, 3, 6, 9, Color::new(0.35, 0.55, 0.85, 0.55)); // wraith
        i.rect(3, 5, 2, 5, Color::new(0.3, 0.5, 0.8, 0.4));
        i.rect(11, 5, 2, 5, Color::new(0.3, 0.5, 0.8, 0.4));
        i.px(6, 6, Color::new(0.9, 1.0, 1.0, 1.0));
        i.px(9, 6, Color::new(0.9, 1.0, 1.0, 1.0));
        mons.push((Kind::Wraith, i.tex()));
        let mut i = Img::new();
        i.disc(8, 6, 4, Color::new(0.5, 0.4, 0.3, 1.0)); // ogre
        i.rect(4, 9, 8, 6, Color::new(0.45, 0.36, 0.27, 1.0));
        i.px(6, 5, Color::new(1.0, 0.3, 0.2, 1.0));
        i.px(10, 5, Color::new(1.0, 0.3, 0.2, 1.0));
        i.rect(12, 4, 2, 8, Color::new(0.4, 0.3, 0.2, 1.0)); // club
        mons.push((Kind::Ogre, i.tex()));
        let mut i = mk_door(&mut rng, None); // mimic
        i.rect(4, 8, 8, 2, Color::new(0.9, 0.9, 0.9, 1.0));
        i.rect(5, 7, 1, 1, Color::new(0.9, 0.9, 0.9, 1.0));
        i.rect(8, 7, 1, 1, Color::new(0.9, 0.9, 0.9, 1.0));
        i.rect(10, 7, 1, 1, Color::new(0.9, 0.9, 0.9, 1.0));
        i.px(5, 5, Color::new(0.9, 0.2, 0.2, 1.0));
        i.px(10, 5, Color::new(0.9, 0.2, 0.2, 1.0));
        mons.push((Kind::Mimic, i.tex()));
        let mut i = Img::new();
        i.rect(5, 6, 6, 8, Color::new(0.25, 0.15, 0.4, 1.0)); // lich
        i.disc(8, 4, 2, Color::new(0.85, 0.85, 0.8, 1.0));
        i.px(7, 4, Color::new(0.3, 1.0, 0.9, 1.0));
        i.px(9, 4, Color::new(0.3, 1.0, 0.9, 1.0));
        i.rect(12, 2, 1, 11, Color::new(0.5, 0.4, 0.25, 1.0));
        i.disc(12, 2, 1, Color::new(0.3, 1.0, 0.9, 1.0));
        mons.push((Kind::Lich, i.tex()));
        let mut i = Img::new();
        i.disc(8, 8, 6, Color::new(0.7, 0.15, 0.2, 1.0)); // boss brute
        i.rect(5, 4, 6, 3, Color::new(0.85, 0.7, 0.2, 1.0));
        i.px(6, 7, Color::new(1.0, 1.0, 0.5, 1.0));
        i.px(10, 7, Color::new(1.0, 1.0, 0.5, 1.0));
        mons.push((Kind::Boss, i.tex()));
        let mut i = Img::new();
        i.disc(8, 9, 5, Color::new(0.65, 0.12, 0.1, 1.0)); // dragon body
        i.disc(11, 5, 3, Color::new(0.75, 0.16, 0.12, 1.0)); // head
        i.px(12, 4, Color::new(1.0, 0.85, 0.2, 1.0));
        i.rect(2, 3, 5, 2, Color::new(0.5, 0.08, 0.08, 1.0)); // wing
        i.rect(1, 5, 3, 3, Color::new(0.55, 0.1, 0.1, 1.0));
        i.rect(13, 9, 2, 1, Color::new(1.0, 0.5, 0.1, 1.0)); // fire breath hint
        mons.push((Kind::Dragon, i.tex()));
    }
    Art {
        wall: wall.tex(),
        floor: floor.tex(),
        corridor: corridor.tex(),
        cave: cave.tex(),
        water: water.tex(),
        lava: lava.tex(),
        doors,
        door_open: door_open.tex(),
        door_plain,
        vault: vault.tex(),
        chest: chest.tex(),
        chest_open: chest_open.tex(),
        fountain: fountain.tex(),
        fountain_used: fountain_used.tex(),
        trap: trap.tex(),
        stairs: stairs.tex(),
        shrine: shrine.tex(),
        captive: captive.tex(),
        hero: hero.tex(),
        mons,
        torch_off: torch_off.tex(),
    }
}
