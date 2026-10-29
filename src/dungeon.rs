use crate::rng::Rng;

pub const MAP_W: usize = 128;
pub const MAP_H: usize = 80;
pub const N_FLOORS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Wall,
    Floor,
    Corridor,
    Cave,
    Water,
    Lava,
    Door, // open (never locked after opening)
    StairsDown,
    Fountain,
    Chest,
    Trap,     // hidden until triggered or seen adjacent
    SecretDoor, // looks like Wall to FOV until opened
    Torch,    // decorative light on wall — walkable? no, it's wall-attached: keep as Wall flag instead
    Shrine,
    Vault,    // big locked door for fetch-quest
    Captive,  // rescue target tile (person stands on floor; tile is floor, entity handles it) — keep Floor, unused marker
}

/// Key colors == door lock tiers. 0 blue < 1 green < 2 red.
pub const N_TIERS: usize = 3;
pub const TIER_NAMES: [&str; N_TIERS] = ["Blue", "Green", "Red"];

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub t: Tile,
    pub locked: i8, // -1 = unlocked door; 0..2 tier; Vault uses 2 + vault flag
    pub is_vault: bool,
    pub trap_armed: bool,
    pub torch: bool, // wall tile with a torch
}

impl Default for Cell {
    fn default() -> Self {
        Cell { t: Tile::Wall, locked: -1, is_vault: false, trap_armed: false, torch: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Room {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}
impl Room {
    pub fn cx(&self) -> usize {
        self.x + self.w / 2
    }
    pub fn cy(&self) -> usize {
        self.y + self.h / 2
    }
    pub fn inner(&self) -> impl Iterator<Item = (usize, usize)> + use<'_> {
        (self.y..self.y + self.h).flat_map(move |y| (self.x..self.x + self.w).map(move |x| (x, y)))
    }
}

pub struct FloorGen {
    pub cells: Vec<Cell>,
    pub w: usize,
    pub h: usize,
    pub entry: (usize, usize),
    pub stairs: (usize, usize),
    pub rooms: Vec<Room>,
    /// (color tier, door pos, key pos) guarantees: key pos reachable using only doors of tier < color.
    pub locked_doors: Vec<(usize, (usize, usize), (usize, usize))>,
    pub chest_positions: Vec<(usize, usize)>,
    pub trap_positions: Vec<(usize, usize)>,
    pub secret_positions: Vec<(usize, usize)>,
    pub fountain_positions: Vec<(usize, usize)>,
    pub torch_positions: Vec<(usize, usize)>,
    pub shrine: Option<(usize, usize)>,
    pub vault: Option<(usize, usize)>,
    pub captive_spot: Option<(usize, usize)>,
}

impl FloorGen {
    pub fn idx(&self, x: usize, y: usize) -> usize {
        y * self.w + x
    }
    pub fn set(&mut self, x: usize, y: usize) -> &mut Cell {
        let i = y * self.w + x;
        &mut self.cells[i]
    }
    pub fn at(&self, x: i32, y: i32) -> Option<&Cell> {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            Some(&self.cells[y as usize * self.w + x as usize])
        } else {
            None
        }
    }
    pub fn at_mut(&mut self, x: i32, y: i32) -> Option<&mut Cell> {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            Some(&mut self.cells[y as usize * self.w + x as usize])
        } else {
            None
        }
    }
    /// Walkable for connectivity purposes (doors passable, liquids not).
    pub fn passable(&self, x: i32, y: i32) -> bool {
        match self.at(x, y) {
            Some(c) => matches!(
                c.t,
                Tile::Floor
                    | Tile::Corridor
                    | Tile::Cave
                    | Tile::Door
                    | Tile::StairsDown
                    | Tile::Fountain
                    | Tile::Chest
                    | Tile::Trap
                    | Tile::Shrine
                    | Tile::Vault
                    | Tile::Captive
            ),
            None => false,
        }
    }
}

/// BFS reachable set from `start`. `open_tiers` = bitset of door tiers we may pass;
/// unlocked doors and secret doors (if allow_secret) always passable. Vault needs tier 2.
pub fn reachable(
    f: &FloorGen,
    start: (usize, usize),
    open_tiers: usize,
) -> Vec<bool> {
    let mut seen = vec![false; f.w * f.h];
    let mut q = std::collections::VecDeque::new();
    if !f.passable(start.0 as i32, start.1 as i32) {
        return seen;
    }
    seen[f.idx(start.0, start.1)] = true;
    q.push_back(start);
    while let Some((x, y)) = q.pop_front() {
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            let c = match f.at(nx, ny) {
                Some(c) => c,
                None => continue,
            };
            if !f.passable(nx, ny) {
                continue;
            }
            if c.locked >= 0 && (open_tiers >> c.locked) & 1 == 0 {
                continue; // locked door of a tier we can't open
            }
            let i = f.idx(nx as usize, ny as usize);
            if !seen[i] {
                seen[i] = true;
                q.push_back((nx as usize, ny as usize));
            }
        }
    }
    seen
}

/// Generate one floor. floor_no: 0..7 (affects style density + difficulty).
pub fn gen_floor(seed: u64, floor_no: usize) -> FloorGen {
    let style = floor_no % 3; // 0 rooms, 1 caves, 2 mixed
    let mut rng = Rng::new(seed ^ (floor_no as u64).wrapping_mul(0x517cC1B727220A95));
    let mut f = FloorGen {
        cells: vec![Cell::default(); MAP_W * MAP_H],
        w: MAP_W,
        h: MAP_H,
        entry: (1, 1),
        stairs: (1, 1),
        rooms: vec![],
        locked_doors: vec![],
        chest_positions: vec![],
        trap_positions: vec![],
        secret_positions: vec![],
        fountain_positions: vec![],
        torch_positions: vec![],
        shrine: None,
        vault: None,
        captive_spot: None,
    };

    let mut rooms: Vec<Room> = vec![];
    let want_rooms = if style == 1 { 4 } else { 10 + rng.range(0, 6) };
    let mut tries = 0;
    while rooms.len() < want_rooms && tries < 400 {
        tries += 1;
        let w = rng.range(6, 16);
        let h = rng.range(5, 12);
        let x = rng.range(2, MAP_W - w - 2);
        let y = rng.range(2, MAP_H - h - 2);
        let r = Room { x, y, w, h };
        if rooms.iter().any(|o| {
            r.x < o.x + o.w + 2 && o.x < r.x + r.w + 2 && r.y < o.y + o.h + 2 && o.y < r.y + r.h + 2
        }) {
            continue;
        }
        rooms.push(r);
    }
    f.rooms = rooms.clone();

    // carve rooms
    for r in &rooms {
        for (x, y) in r.inner() {
            f.set(x, y).t = Tile::Floor;
        }
    }
    // connect rooms with L corridors (MST-ish order + extra loops)
    let mut connected = vec![0usize];
    let mut order: Vec<usize> = (1..rooms.len()).collect();
    // nearest-neighbour chain
    while !order.is_empty() {
        let last = *connected.last().unwrap();
        let mut bi = 0;
        let mut bd = usize::MAX;
        for (i, &o) in order.iter().enumerate() {
            let d = rooms[last].cx().abs_diff(rooms[o].cx()) + rooms[last].cy().abs_diff(rooms[o].cy());
            if d < bd {
                bd = d;
                bi = i;
            }
        }
        let nxt = order.remove(bi);
        let (a, b) = (rooms[last], rooms[nxt]);
        carve_corridor(&mut f, &mut rng, a.cx(), a.cy(), b.cx(), b.cy());
        connected.push(nxt);
    }
    // a few loops for interesting layouts
    let extras = rng.range(1, 4);
    for _ in 0..extras {
        if rooms.len() > 2 {
            let a = rng.range(0, rooms.len());
            let b = rng.range(0, rooms.len());
            if a != b {
                carve_corridor(&mut f, &mut rng, rooms[a].cx(), rooms[a].cy(), rooms[b].cx(), rooms[b].cy());
            }
        }
    }

    // caves: cellular automata blobs, then connect to nearest room
    if style != 0 {
        let n_blobs = if style == 1 { 5 } else { 2 };
        for _ in 0..n_blobs {
            let bx = rng.range(10, MAP_W - 10);
            let by = rng.range(10, MAP_H - 10);
            let br = rng.range(4, 9);
            carve_cave(&mut f, &mut rng, bx, by, br);
            if !rooms.is_empty() {
                let r = rng.pick(&rooms);
                // dig a corridor from blob centre to room centre if not already open
                carve_line(&mut f, bx, by, r.cx(), r.cy());
            }
        }
    }

    // water / lava puddles in corners of big floors
    let n_pools = rng.range(0, 3 + floor_no / 2);
    for _ in 0..n_pools {
        let kind = if floor_no >= 4 && rng.chance(0.4) { Tile::Lava } else { Tile::Water };
        let x = rng.range(3, MAP_W - 4);
        let y = rng.range(3, MAP_H - 4);
        let r = rng.range(1, 3);
        for dy in -(r as i32)..=r as i32 {
            for dx in -(r as i32)..=r as i32 {
                if dx * dx + dy * dy <= (r * r) as i32 {
                    if let Some(c) = f.at_mut(x as i32 + dx, y as i32 + dy) {
                        if c.t == Tile::Floor || c.t == Tile::Cave {
                            c.t = kind;
                        }
                    }
                }
            }
        }
    }

    // entry = first room centre; stairs = farthest room/cave floor from entry
    if let Some(r) = rooms.first() {
        f.entry = (r.cx(), r.cy());
    } else {
        f.entry = find_any_floor(&f, &mut rng);
    }
    let all = reachable(&f, f.entry, (1 << N_TIERS) - 1);
    let mut best = f.entry;
    let mut bestd = 0usize;
    for y in 0..f.h {
        for x in 0..f.w {
            if all[f.idx(x, y)] {
                let d = x.abs_diff(f.entry.0) + y.abs_diff(f.entry.1);
                if d > bestd {
                    bestd = d;
                    best = (x, y);
                }
            }
        }
    }
    f.stairs = best;
    f.set(best.0, best.1).t = Tile::StairsDown;

    // doors: corridor tiles with wall on two opposite sides & floor on the other two
    let mut door_spots: Vec<(usize, usize)> = vec![];
    for y in 1..f.h - 1 {
        for x in 1..f.w - 1 {
            let i = f.idx(x, y);
            if !matches!(f.cells[i].t, Tile::Corridor | Tile::Cave | Tile::Floor) {
                continue;
            }
            let wall_ns = !f.passable(x as i32, y as i32 - 1) && !f.passable(x as i32, y as i32 + 1);
            let wall_ew = !f.passable(x as i32 - 1, y as i32) && !f.passable(x as i32 + 1, y as i32);
            let floor_ns = f.passable(x as i32, y as i32 - 1) && f.passable(x as i32, y as i32 + 1);
            let floor_ew = f.passable(x as i32 - 1, y as i32) && f.passable(x as i32 + 1, y as i32);
            if (wall_ns && floor_ew) || (wall_ew && floor_ns) {
                if rng.chance(0.12 + 0.02 * floor_no as f32) {
                    door_spots.push((x, y));
                }
            }
        }
    }
    for (x, y) in &door_spots {
        f.set(*x, *y).t = Tile::Door;
    }

    // Tiered locked doors, one per tier, placed 0→2 by induction:
    // when placing tier c, no door of tier >= c exists yet, so
    // R = reachable(entry, doors of tier < c passable) is exactly "what the hero
    // can see with keys for all shallower tiers". We pick a door in R, cut off its
    // far side F (must be 15..=half of R, must not contain the stairs or any
    // previously-placed key), and place the tier-c key in R \ F. The hero can
    // always grab the key before opening its door.
    let mut placed_keys: Vec<(usize, usize)> = vec![];
    let mut tier_doors: Vec<(usize, (usize, usize))> = vec![];
    for tier in 0..N_TIERS {
        let r_mask = (1usize << tier) - 1; // tiers < tier
        let rin = reachable(&f, f.entry, r_mask);
        let total_in = rin.iter().filter(|&&b| b).count();
        let mut cand: Vec<(usize, usize, Vec<bool>)> = vec![];
        for (x, y) in &door_spots {
            let i = f.idx(*x, *y);
            if f.cells[i].locked >= 0 || !rin[i] {
                continue;
            }
            let mut cells2 = f.cells.clone();
            cells2[i].locked = tier as i8;
            let fin = FloorGen { cells: cells2, ..floor_shell(&f) };
            let inside = reachable_pub(&fin, f.entry, r_mask);
            let far: Vec<bool> = (0..f.cells.len()).map(|j| rin[j] && !inside[j]).collect();
            let nf = far.iter().filter(|&&b| b).count();
            if nf < 15 || nf * 2 >= total_in {
                continue;
            }
            if far[f.idx(f.stairs.0, f.stairs.1)] {
                continue; // stairs stay keyless — always progressable
            }
            if placed_keys.iter().any(|k| far[f.idx(k.0, k.1)]) {
                continue; // never strand an earlier key
            }
            cand.push((*x, *y, far));
        }
        if cand.is_empty() {
            continue;
        }
        let (dx, dy, far) = rng.pick(&cand);
        f.set(dx, dy).locked = tier as i8;
        tier_doors.push((tier, (dx, dy)));
        let key_spot = loop_pick(&mut rng, &f, |x, y| {
            let j = f.idx(x, y);
            rin[j]
                && !far[j]
                && !placed_keys.iter().any(|k| *k == (x, y))
                && matches!(f.cells[j].t, Tile::Floor | Tile::Cave)
        });
        if let Some(kp) = key_spot {
            placed_keys.push(kp);
            f.locked_doors.push((tier, (dx, dy), kp));
        }
    }

    // Vault: the red door (tier 2) is the vault door; vault chest sits behind it.
    if let Some((_, pos)) = tier_doors.iter().find(|(t, _)| *t == 2) {
        if let Some(c) = f.at_mut(pos.0 as i32, pos.1 as i32) {
            c.is_vault = true;
        }
        f.vault = Some(*pos);
        let i = f.idx(pos.0, pos.1);
        let r2 = reachable(&f, f.entry, (1 << N_TIERS) - 1);
        // pick a floor cell deeper than the vault door for the vault chest
        let mut g = f.cells.clone();
        g[i].locked = 3; // vault "locked" for reachability test only (out of tier range)
        let fin2 = FloorGen { cells: g, ..floor_shell(&f) };
        let inside2 = reachable_pub(&fin2, f.entry, (1 << N_TIERS) - 1);
        let far2: Vec<bool> = (0..f.cells.len()).map(|j| r2[j] && !inside2[j]).collect();
        let vault_spot = loop_pick(&mut rng, &f, |x, y| far2[f.idx(x, y)] && matches!(f.cells[f.idx(x, y)].t, Tile::Floor | Tile::Cave));
        if let Some(vs) = vault_spot {
            f.set(vs.0, vs.1).t = Tile::Chest;
            if !f.chest_positions.contains(&vs) {
                f.chest_positions.push(vs);
            }
            f.vault = Some(vs); // quest target is the chest itself
        }
    }

    // Final connectivity repair: dig straight corridors until every passable
    // cell is reachable from entry (all doors passable in this test).
    let mut guard = 0;
    loop {
        guard += 1;
        let full = reachable(&f, f.entry, (1 << N_TIERS) - 1);
        let mut fixed = 0;
        for y in 1..f.h - 1 {
            for x in 1..f.w - 1 {
                let i = f.idx(x, y);
                if f.passable(x as i32, y as i32) && !full[i] {
                    let _ = &full;
                    { let e = f.entry; dig_line(&mut f, e, (x, y)); }
                    fixed += 1;
                    if fixed >= 8 {
                        break;
                    }
                }
            }
            if fixed >= 8 {
                break;
            }
        }
        if fixed == 0 || guard > 30 {
            break;
        }
    }

    // secret doors: walls with floor on both horizontal or vertical sides, not in door_spots
    let mut n_secret = 0;
    for _ in 0..40 {
        if n_secret >= 2 + floor_no / 3 {
            break;
        }
        let x = rng.range(2, MAP_W - 2);
        let y = rng.range(2, MAP_H - 2);
        let i = f.idx(x, y);
        if f.cells[i].t != Tile::Wall {
            continue;
        }
        let ew = f.passable(x as i32 - 1, y as i32) && f.passable(x as i32 + 1, y as i32);
        let ns = f.passable(x as i32, y as i32 - 1) && f.passable(x as i32, y as i32 + 1);
        if ew || ns {
            f.cells[i].t = Tile::SecretDoor;
            f.secret_positions.push((x, y));
            n_secret += 1;
        }
    }

    // fountains, chests, traps, torches
    let n_f = rng.range(1, 3);
    for _ in 0..n_f {
        if let Some(p) = loop_pick(&mut rng, &f, |x, y| {
            let j = f.idx(x, y);
            f.cells[j].t == Tile::Floor || f.cells[j].t == Tile::Cave
        }) {
            f.set(p.0, p.1).t = Tile::Fountain;
            f.fountain_positions.push(p);
        }
    }
    let n_c = rng.range(2, 5);
    for _ in 0..n_c {
        if let Some(p) = loop_pick(&mut rng, &f, |x, y| {
            let j = f.idx(x, y);
            (f.cells[j].t == Tile::Floor || f.cells[j].t == Tile::Cave) && j != f.idx(f.entry.0, f.entry.1)
        }) {
            f.set(p.0, p.1).t = Tile::Chest;
            f.chest_positions.push(p);
        }
    }
    let n_t = rng.range(2, 4 + floor_no);
    for _ in 0..n_t {
        if let Some(p) = loop_pick(&mut rng, &f, |x, y| {
            let j = f.idx(x, y);
            (f.cells[j].t == Tile::Floor || f.cells[j].t == Tile::Cave || f.cells[j].t == Tile::Corridor)
                && f.cells[j].locked < 0
        }) {
            let i = f.idx(p.0, p.1);
            f.cells[i].t = Tile::Trap;
            f.cells[i].trap_armed = true;
            f.trap_positions.push(p);
        }
    }
    // torches on walls adjacent to floors near rooms
    let n_to = rng.range(8, 16);
    for _ in 0..n_to {
        if let Some((x, y)) = loop_pick(&mut rng, &f, |x, y| {
            let i = f.idx(x, y);
            f.cells[i].t == Tile::Wall && !f.cells[i].torch && {
                let n = [(x as i32 - 1, y as i32), (x as i32 + 1, y as i32), (x as i32, y as i32 - 1), (x as i32, y as i32 + 1)];
                n.iter().any(|&(nx, ny)| f.passable(nx, ny))
            }
        }) {
            let i = f.idx(x, y);
            f.cells[i].torch = true;
            f.torch_positions.push((x, y));
        }
    }

    // quest fixtures
    {
        // shrine somewhere in the far half
        if let Some(p) = loop_pick(&mut rng, &f, |x, y| {
            let i = f.idx(x, y);
            (f.cells[i].t == Tile::Floor || f.cells[i].t == Tile::Cave)
                && x.abs_diff(f.entry.0) + y.abs_diff(f.entry.1) > 20
        }) {
            f.set(p.0, p.1).t = Tile::Shrine;
            f.shrine = Some(p);
        }
    }
    {
        let rin_all = reachable(&f, f.entry, (1 << N_TIERS) - 1);
        if let Some(p) = loop_pick(&mut rng, &f, |x, y| {
            let i = f.idx(x, y);
            (f.cells[i].t == Tile::Floor || f.cells[i].t == Tile::Cave) && rin_all[i]
        }) {
            f.captive_spot = Some(p); // entity placed later
        }
    }
    f
}

fn floor_shell(f: &FloorGen) -> FloorGen {
    FloorGen {
        cells: vec![Cell::default(); 1],
        w: f.w,
        h: f.h,
        entry: f.entry,
        stairs: f.stairs,
        rooms: f.rooms.clone(),
        locked_doors: f.locked_doors.clone(),
        chest_positions: f.chest_positions.clone(),
        trap_positions: f.trap_positions.clone(),
        secret_positions: f.secret_positions.clone(),
        fountain_positions: f.fountain_positions.clone(),
        torch_positions: f.torch_positions.clone(),
        shrine: f.shrine,
        vault: f.vault,
        captive_spot: f.captive_spot,
    }
}

fn style_ok(_rng: &Rng) -> bool {
    true
}

fn reachable_pub(f: &FloorGen, start: (usize, usize), tiers: usize) -> Vec<bool> {
    reachable(f, start, tiers)
}

impl FloorGen {
    fn clone_for_test(&self) -> FloorGen {
        FloorGen {
            cells: self.cells.clone(),
            w: self.w,
            h: self.h,
            entry: self.entry,
            stairs: self.stairs,
            rooms: self.rooms.clone(),
            locked_doors: self.locked_doors.clone(),
            chest_positions: self.chest_positions.clone(),
            trap_positions: self.trap_positions.clone(),
            secret_positions: self.secret_positions.clone(),
            fountain_positions: self.fountain_positions.clone(),
            torch_positions: self.torch_positions.clone(),
            shrine: self.shrine,
            vault: self.vault,
            captive_spot: self.captive_spot,
        }
    }
}

fn find_any_floor(f: &FloorGen, rng: &mut Rng) -> (usize, usize) {
    for _ in 0..2000 {
        let x = rng.range(1, f.w - 1);
        let y = rng.range(1, f.h - 1);
        if f.passable(x as i32, y as i32) {
            return (x, y);
        }
    }
    (1, 1)
}

fn loop_pick(f_pred_rng: &mut Rng, f: &FloorGen, pred: impl Fn(usize, usize) -> bool) -> Option<(usize, usize)> {
    for _ in 0..3000 {
        let x = f_pred_rng.range(1, f.w - 1);
        let y = f_pred_rng.range(1, f.h - 1);
        if pred(x, y) {
            return Some((x, y));
        }
    }
    None
}

fn carve_line(f: &mut FloorGen, x0: usize, y0: usize, x1: usize, y1: usize) {
    let mut x = x0 as i32;
    let mut y = y0 as i32;
    let dx = (x1 as i32 - x).abs();
    let dy = -(y1 as i32 - y).abs();
    let sx = if x < x1 as i32 { 1 } else { -1 };
    let sy = if y < y1 as i32 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x > 0 && y > 0 && (x as usize) < f.w - 1 && (y as usize) < f.h - 1 {
            let i = f.idx(x as usize, y as usize);
            if matches!(f.cells[i].t, Tile::Wall | Tile::Water | Tile::Lava) {
                f.cells[i].t = Tile::Corridor;
            }
        }
        if x == x1 as i32 && y == y1 as i32 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn connect_if_needed(
    _f: &FloorGen,
    _carve: &mut impl FnMut(usize, usize, usize, usize),
    _a: (usize, usize),
    _b: (usize, usize),
) {
}

fn carve_corridor(f: &mut FloorGen, rng: &mut Rng, x0: usize, y0: usize, x1: usize, y1: usize) {
    if rng.chance(0.5) {
        dig_h(f, x0, x1, y0);
        dig_v(f, y0, y1, x1);
    } else {
        dig_v(f, y0, y1, x0);
        dig_h(f, x0, x1, y1);
    }
}
fn dig_h(f: &mut FloorGen, xa: usize, xb: usize, y: usize) {
    for x in xa.min(xb)..=xa.max(xb) {
        let i = f.idx(x, y);
        if matches!(f.cells[i].t, Tile::Wall | Tile::Water | Tile::Lava) {
            f.cells[i].t = Tile::Corridor;
        }
    }
}
fn dig_v(f: &mut FloorGen, ya: usize, yb: usize, x: usize) {
    for y in ya.min(yb)..=ya.max(yb) {
        let i = f.idx(x, y);
        if matches!(f.cells[i].t, Tile::Wall | Tile::Water | Tile::Lava) {
            f.cells[i].t = Tile::Corridor;
        }
    }
}

fn carve_cave(f: &mut FloorGen, rng: &mut Rng, cx: usize, cy: usize, r: usize) {
    let r2 = r * r;
    for dy in -(r as i32)..=r as i32 {
        for dx in -(r as i32)..=r as i32 {
            let x = cx as i32 + dx;
            let y = cy as i32 + dy;
            if x <= 0 || y <= 0 || x as usize >= f.w - 1 || y as usize >= f.h - 1 {
                continue;
            }
            let d = dx * dx + dy * dy;
            if (d as usize) <= r2 && rng.next_f32() > (d as f32 / r2 as f32) * 0.6 {
                let i = f.idx(x as usize, y as usize);
                if matches!(f.cells[i].t, Tile::Wall) {
                    f.cells[i].t = Tile::Cave;
                }
            }
        }
    }
}

fn dig_line(f: &mut FloorGen, a: (usize, usize), b: (usize, usize)) {
    // L-shaped (orthogonal-only) so the pocket is 4-connected to the main area
    if (a.0 + a.1 + b.0 + b.1) % 2 == 0 {
        dig_h(f, a.0, b.0, a.1);
        dig_v(f, a.1, b.1, b.0);
    } else {
        dig_v(f, a.1, b.1, a.0);
        dig_h(f, a.0, b.0, b.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_before_door(seed: u64) {
        for floor_no in 0..N_FLOORS {
            let f = gen_floor(seed, floor_no);
            // 1) whole floor connected with all doors passable
            let all = reachable(&f, f.entry, (1 << N_TIERS) - 1);
            let passable_count =
                (0..f.cells.len()).filter(|&i| f.passable((i % f.w) as i32, (i / f.w) as i32)).count();
            assert_eq!(
                all.iter().filter(|&&b| b).count(),
                passable_count,
                "seed {seed} floor {floor_no}: disconnected passable cells"
            );
            // 2) each locked door: its key reachable before opening any door of tier >= its tier... 
            //    stronger check per tier: for every tier c door, key reachable with tiers < c only.
            for (tier, _door, key) in &f.locked_doors {
                let open = if *tier == 0 { 0 } else { (1 << *tier) - 1 };
                let pre = reachable(&f, f.entry, open);
                assert!(pre[f.idx(key.0, key.1)], "seed {seed} floor {floor_no}: tier-{tier} key unreachable before door");
            }
            // 3) stairs reachable even with no keys? (must not require keys — guarantees progress)
            let no_keys = reachable(&f, f.entry, 0);
            assert!(no_keys[f.idx(f.stairs.0, f.stairs.1)], "seed {seed} floor {floor_no}: stairs needs a key");
            // 4) vault/quest fixtures reachable with all tiers
            if let Some(v) = f.vault {
                assert!(all[f.idx(v.0, v.1)]);
            }
            if let Some(s) = f.shrine {
                assert!(all[f.idx(s.0, s.1)]);
            }
            if let Some(c) = f.captive_spot {
                assert!(all[f.idx(c.0, c.1)]);
            }
        }
    }

    #[test]
    fn floors_valid_many_seeds() {
        for s in [1u64, 7, 42, 1234, 99991, 31337, 2024, 8888] {
            key_before_door(s);
        }
    }
    #[test]
    fn reproducible() {
        let a = gen_floor(555, 3);
        let b = gen_floor(555, 3);
        assert_eq!(a.entry, b.entry);
        assert_eq!(a.stairs, b.stairs);
        assert!(a.cells.iter().zip(b.cells.iter()).all(|(x, y)| x.t == y.t && x.locked == y.locked));
    }
    #[test]
    fn has_locked_doors_sometimes_and_room_count() {
        let mut total_locked = 0;
        for s in 0..12u64 {
            let f = gen_floor(s * 131 + 5, (s as usize) % N_FLOORS);
            total_locked += f.locked_doors.len();
            assert!(f.rooms.len() >= 4, "too few rooms on seed {s}");
        }
        assert!(total_locked >= 8, "locked doors never generated: {total_locked}");
    }
}
