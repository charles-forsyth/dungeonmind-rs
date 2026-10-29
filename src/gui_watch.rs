// gui_watch: Watch mode (showcase episode + background trainer), animation
// tweening, zoom/pan camera input, thought overlay, recap card, hints.
// (included by gui_draw.rs)

pub const MAX_ZOOM: f32 = 4.0;
pub const WATCH_ZOOM: f32 = 3.0;
pub const SHOWCASE_MAX_TURNS: u32 = 2000;

pub struct Opts {
    pub seed: u64,
    pub fixed: bool,
    pub watch: bool,
    pub zoom: Option<f32>, // Some(0.0) = fit whole floor
    pub speed: Option<Speed>,
    pub thoughts: bool,
    pub hint: bool,
    pub seek: Option<String>,
}

pub fn parse_speed(s: &str) -> Option<Speed> {
    match s.to_ascii_lowercase().as_str() {
        "0" | "pause" | "paused" => Some(Speed::Paused),
        "0.25" | ".25" => Some(Speed::Q25),
        "0.5" | ".5" => Some(Speed::Q50),
        "1" | "1x" => Some(Speed::X1),
        "4" | "4x" => Some(Speed::X4),
        "16" | "16x" => Some(Speed::X16),
        "max" => Some(Speed::Max),
        _ => None,
    }
}

pub fn parse_opts() -> Opts {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut o = Opts { seed: 1337, fixed: false, watch: true, zoom: None, speed: None, thoughts: true, hint: true, seek: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => o.seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(o.seed),
            "--fixed-seed" => o.fixed = true,
            "--watch" => o.watch = true,
            "--no-watch" | "--classic" => o.watch = false,
            "--zoom" => {
                o.zoom = it.next().map(|s| if s == "fit" { 0.0 } else { s.parse().unwrap_or(WATCH_ZOOM) });
            }
            "--speed" => o.speed = it.next().and_then(|s| parse_speed(s)),
            "--no-thoughts" => o.thoughts = false,
            "--no-hint" => o.hint = false,
            "--seek" => o.seek = it.next().cloned(),
            _ => {}
        }
    }
    o
}

/// Quantize dynamic font sizes: macroquad caches a glyph atlas per size, and
/// churning through many fractional sizes corrupts it on software GL.
fn qsize(s: f32) -> f32 {
    ((s / 4.0).round() * 4.0).max(8.0)
}

fn shadow_text(t: &str, x: f32, y: f32, size: f32, col: Color) {
    draw_text(t, x + 1.0, y + 1.0, size, Color::new(0.0, 0.0, 0.0, col.a * 0.85));
    draw_text(t, x, y, size, col);
}

fn lerp_col(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

/// red (bad) -> yellow -> green (good)
fn q_tint(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let red = Color::new(0.95, 0.25, 0.25, 1.0);
    let yel = Color::new(0.98, 0.85, 0.25, 1.0);
    let grn = Color::new(0.3, 0.95, 0.4, 1.0);
    if t < 0.5 {
        lerp_col(red, yel, t * 2.0)
    } else {
        lerp_col(yel, grn, (t - 0.5) * 2.0)
    }
}

impl Game {
    pub fn view_size() -> (f32, f32) {
        ((screen_width() - PANEL_W).max(320.0), (screen_height() - HUD_H).max(240.0))
    }

    pub fn fit_zoom(&self) -> f32 {
        let (vw, vh) = Game::view_size();
        let fx = vw / (self.world.map.w as f32 * TILE);
        let fy = vh / (self.world.map.h as f32 * TILE);
        fx.min(fy).max(0.15)
    }

    pub fn set_speed(&mut self, sp: Speed) {
        self.speed = sp;
        if sp != Speed::Paused {
            self.last_speed = sp;
        }
    }

    pub fn speed_step(&mut self, dir: i32) {
        let ladder = [Speed::Q25, Speed::Q50, Speed::X1, Speed::X4, Speed::X16, Speed::Max];
        let cur = ladder.iter().position(|s| *s == self.speed).unwrap_or(1) as i32;
        let n = (cur + dir).clamp(0, ladder.len() as i32 - 1) as usize;
        self.set_speed(ladder[n]);
    }

    pub fn eps_now(&self) -> f32 {
        self.trainer.as_ref().map(|t| t.eps()).unwrap_or(self.agent.eps)
    }
    pub fn qsize_now(&self) -> usize {
        self.trainer.as_ref().map(|t| t.q_size()).unwrap_or(self.agent.q.len())
    }

    /// Turn Watch mode on/off. On: trainer thread takes a copy of the agent and
    /// learns at full speed; the screen plays a showcase episode.
    pub fn set_watch(&mut self, on: bool) {
        if on == self.watch {
            return;
        }
        if on {
            let mut cfg = self.cfg.clone();
            cfg.seed_mode_fixed = self.seed_fixed;
            cfg.seed = self.cur_seed;
            self.trainer = Some(Trainer::start(self.agent.clone(), cfg, 100));
            self.watch = true;
            self.log.push("WATCH: training runs full speed in the background.".into());
            self.trim_log();
            self.start_showcase(None);
        } else {
            self.watch = false;
            if let Some(tr) = self.trainer.take() {
                self.agent = tr.stop();
                self.episodes_total = self.agent.episodes;
            }
            self.showcase = None;
            self.recap = None;
            self.sc_pause = 0.0;
            self.log.push("Classic mode: the on-screen hero is the one that learns.".into());
            self.trim_log();
            self.world = World::new(self.cur_seed);
            self.ep_reward = 0.0;
            self.ep_turns = 0;
            self.drain_events();
        }
    }

    pub fn start_showcase(&mut self, seed: Option<u64>) {
        let tr = match &self.trainer {
            Some(t) => t,
            None => return,
        };
        let snap = tr.latest();
        let seed = seed.unwrap_or_else(|| if self.seed_fixed { self.cur_seed } else { self.ep_rng.next_u64() });
        self.sc_no += 1;
        self.world = World::new(seed);
        let mut sc = Showcase::new(snap, seed, self.sc_no);
        sc.think(&self.world);
        self.showcase = Some(sc);
        self.recap = None;
        self.sc_pause = 0.0;
        self.ep_reward = 0.0;
        self.ep_turns = 0;
        self.floats.clear();
        self.parts.clear();
        self.dice.clear();
        self.sync_thought();
        self.drain_events();
        self.log.push(format!(
            "— Showcase #{} starts (policy of training episode {}) —",
            self.sc_no,
            self.showcase.as_ref().map(|s| s.snap.episodes).unwrap_or(0)
        ));
        self.trim_log();
    }

    fn sync_thought(&mut self) {
        if let Some(sc) = &self.showcase {
            if let Some(t) = &sc.thought {
                self.cur_q = t.q;
                self.cur_legal = t.legal;
                self.cur_action = t.action;
                self.cur_why = t.why.clone();
            }
        }
    }

    pub fn pump_trainer(&mut self) {
        if let Some(tr) = &self.trainer {
            let stats = tr.drain();
            self.episodes_total = tr.episodes();
            for st in stats {
                self.push_hist(&st);
            }
        }
    }

    /// One showcase turn (execute the pending decision, then plan the next).
    pub fn showcase_turn(&mut self) {
        let fast = self.speed.is_fast();
        let (evs, done) = {
            let sc = match self.showcase.as_mut() {
                Some(s) => s,
                None => return,
            };
            let r = sc.act(&mut self.world);
            if !r.1 && sc.turns >= SHOWCASE_MAX_TURNS {
                sc.done = true;
                (r.0, true)
            } else {
                r
            }
        };
        if fast {
            // fast-forward: keep the log/FX quiet
        } else {
            self.handle_events(evs);
        }
        if let Some(sc) = &self.showcase {
            self.ep_reward = sc.reward;
            self.ep_turns = sc.turns;
        }
        if done {
            let (rc, no) = match &self.showcase {
                Some(sc) => (sc.recap(&self.world), sc.episode_no),
                None => return,
            };
            self.log.push(format!(
                "— Showcase #{}: floor {}, {} kills, {} turns — {}",
                no, rc.floor, rc.kills, rc.turns, rc.cause
            ));
            self.trim_log();
            self.recap = Some(rc);
            self.sc_pause = 5.4;
        } else if let Some(sc) = self.showcase.as_mut() {
            sc.think(&self.world);
            self.sync_thought();
        }
    }

    /// Smoothly move drawn positions toward simulation positions.
    pub fn sync_anims(&mut self, dt: f32) {
        let key = (self.world.seed, self.world.floor_no);
        if key != self.anim_key {
            self.anim_key = key;
            self.hero_anim = Anim::at(self.world.hero.x, self.world.hero.y);
            self.mon_anim = self.world.monsters.iter().map(|m| Anim::at(m.x, m.y)).collect();
            self.cam = Vec2::ZERO;
            return;
        }
        while self.mon_anim.len() < self.world.monsters.len() {
            let m = &self.world.monsters[self.mon_anim.len()];
            self.mon_anim.push(Anim::at(m.x, m.y));
        }
        self.mon_anim.truncate(self.world.monsters.len());
        let iv = watch::turn_interval(self.speed.mult());
        let iv = if iv.is_finite() { iv } else { 0.3 };
        let rate = (4.5 / iv).clamp(7.0, 70.0);
        let snap_all = self.speed.is_fast();
        fn ease(a: &mut Anim, tgt: Vec2, rate: f32, dt: f32, snap: bool) {
            let d = tgt - a.pos;
            if snap || d.length() > 2.6 {
                a.pos = tgt;
            } else {
                a.pos += d * (1.0 - (-rate * dt).exp());
                if d.length() < 0.004 {
                    a.pos = tgt;
                }
            }
            a.flash = (a.flash - dt * 2.4).max(0.0);
            if let Some(l) = a.lunge.as_mut() {
                l.t += dt;
                if l.t >= l.dur {
                    a.lunge = None;
                }
            }
        }
        ease(&mut self.hero_anim, Vec2::new(self.world.hero.x as f32, self.world.hero.y as f32), rate, dt, snap_all);
        for (i, m) in self.world.monsters.iter().enumerate() {
            if m.x < 0 || m.y < 0 {
                continue;
            }
            ease(&mut self.mon_anim[i], Vec2::new(m.x as f32, m.y as f32), rate, dt, snap_all);
        }
        for d in self.dice.iter_mut() {
            d.life -= dt;
        }
        self.dice.retain(|d| d.life > 0.0);
    }

    pub fn clamp_cam(&mut self, zoom: f32) {
        let (vw, vh) = Game::view_size();
        let (mw, mh) = (self.world.map.w as f32 * TILE, self.world.map.h as f32 * TILE);
        let (hx, hy) = (vw / 2.0 / zoom, vh / 2.0 / zoom);
        self.cam.x = if mw <= hx * 2.0 { mw / 2.0 } else { self.cam.x.clamp(hx, mw - hx) };
        self.cam.y = if mh <= hy * 2.0 { mh / 2.0 } else { self.cam.y.clamp(hy, mh - hy) };
    }

    /// Wheel / +- / drag / double-click handling for the camera.
    pub fn camera_input(&mut self, dt: f32) {
        let (vw, vh) = Game::view_size();
        let (mx, my) = mouse_position();
        let in_view = mx >= 0.0 && my >= 0.0 && mx < vw && my < vh;
        let fit = self.fit_zoom();
        let (_, wy) = mouse_wheel();
        if wy != 0.0 && in_view {
            self.zoom_target *= 1.2f32.powf(wy.signum());
        }
        if is_key_down(KeyCode::Equal) || is_key_down(KeyCode::KpAdd) {
            self.zoom_target *= (1.6 * dt).exp();
        }
        if is_key_down(KeyCode::Minus) || is_key_down(KeyCode::KpSubtract) {
            self.zoom_target *= (-1.6 * dt).exp();
        }
        if is_key_pressed(KeyCode::Z) {
            self.zoom_target = WATCH_ZOOM;
            self.follow = true;
        }
        if is_key_pressed(KeyCode::F) || is_key_pressed(KeyCode::Home) {
            self.follow = true;
        }
        self.zoom_target = self.zoom_target.clamp(fit, MAX_ZOOM);
        // mouse drag pans, double click = close-up
        if is_mouse_button_pressed(MouseButton::Left) && in_view {
            self.drag_on = true;
            self.drag_moved = false;
            self.drag_start = Vec2::new(mx, my);
            self.drag_last = Vec2::new(mx, my);
        }
        if self.drag_on && is_mouse_button_down(MouseButton::Left) {
            let p = Vec2::new(mx, my);
            if !self.drag_moved && (p - self.drag_start).length() > 5.0 {
                self.drag_moved = true;
                self.follow = false;
            }
            if self.drag_moved {
                let d = p - self.drag_last;
                self.cam -= d / self.zoom.max(0.1);
            }
            self.drag_last = p;
        }
        if self.drag_on && is_mouse_button_released(MouseButton::Left) {
            self.drag_on = false;
            if !self.drag_moved {
                if self.time - self.last_click < 0.38 {
                    self.zoom_target = WATCH_ZOOM;
                    self.follow = true;
                    self.last_click = -10.0;
                } else {
                    self.last_click = self.time;
                }
            }
        }
        let k = 1.0 - (-9.0 * dt).exp();
        self.zoom += (self.zoom_target - self.zoom) * k;
        if (self.zoom_target - self.zoom).abs() < 0.0008 {
            self.zoom = self.zoom_target;
        }
        // keep the eased zoom inside the legal range if the window was resized
        self.zoom = self.zoom.clamp(fit.min(self.zoom_target), MAX_ZOOM);
    }
}

// ----------------------------------------------------------------- drawing

fn view_origin(g: &Game, zoom: f32) -> Vec2 {
    let (vw, vh) = Game::view_size();
    let shake = if g.shake > 0.0 {
        let a = g.time * 47.0;
        Vec2::new(a.sin() * g.shake, (a * 1.3).cos() * g.shake)
    } else {
        Vec2::ZERO
    };
    Vec2::new(vw / 2.0 - g.cam.x * zoom + shake.x * 0.6, vh / 2.0 - g.cam.y * zoom + shake.y * 0.6)
}

fn draw_dice(g: &Game, zoom: f32) {
    let o = view_origin(g, zoom);
    let (vw, vh) = Game::view_size();
    for (i, d) in g.dice.iter().enumerate() {
        let a = (d.life / 0.5).clamp(0.0, 1.0);
        let rolling = d.life > 1.9 - 0.4;
        let shown = if rolling { ((g.time * 55.0) as i32 + i as i32 * 7).rem_euclid(20) + 1 } else { d.d20 };
        let txt = if rolling {
            format!("d20: {}", shown)
        } else {
            format!("d20 {} {:+} vs AC {}: {}", d.d20, d.bonus, d.ac, if d.hit { "HIT" } else { "MISS" })
        };
        let fs = 15.0;
        let tw = measure_text(&txt, None, fs as u16, 1.0).width;
        let ts = TILE * zoom;
        let cx = (d.x * zoom + o.x).clamp(tw / 2.0 + 10.0, vw - tw / 2.0 - 10.0);
        let cy = ((d.y + TILE) * zoom + o.y + 8.0).clamp(20.0, vh - 20.0) + (i % 3) as f32 * 0.0 * ts;
        let edge = if d.hero { Color::new(0.9, 0.75, 0.3, a) } else { Color::new(0.85, 0.35, 0.35, a) };
        draw_rectangle(cx - tw / 2.0 - 7.0, cy - 14.0, tw + 14.0, 21.0, Color::new(0.04, 0.03, 0.06, 0.88 * a));
        draw_rectangle_lines(cx - tw / 2.0 - 7.0, cy - 14.0, tw + 14.0, 21.0, 1.5, edge);
        let col = if rolling {
            Color::new(0.95, 0.95, 0.95, a)
        } else if d.hit {
            Color::new(0.5, 1.0, 0.55, a)
        } else {
            Color::new(0.8, 0.8, 0.85, a)
        };
        draw_text(&txt, cx - tw / 2.0, cy + 1.0, fs, col);
    }
}

fn draw_thought(g: &Game, zoom: f32) {
    if !g.thought_on || zoom < 1.5 || g.world.dead && g.recap.is_some() {
        return;
    }
    let (vw, _vh) = Game::view_size();
    let o = view_origin(g, zoom);
    let ts = TILE * zoom;
    let hp = g.hero_anim.pos;
    let hcx = (hp.x * TILE + TILE / 2.0) * zoom + o.x;
    let hcy = (hp.y * TILE + TILE / 2.0) * zoom + o.y;
    // arrows on neighbouring tiles, tinted by the Q-value of moving there
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for a in 0..8 {
        if g.cur_legal[a] {
            lo = lo.min(g.cur_q[a]);
            hi = hi.max(g.cur_q[a]);
        }
    }
    let span = hi - lo;
    for a in 0..8 {
        if !g.cur_legal[a] {
            continue;
        }
        let (dx, dy) = dungeonmind::qlearn::DIRS[a];
        let dv = Vec2::new(dx as f32, dy as f32).normalize();
        let c = Vec2::new(hcx + dx as f32 * ts, hcy + dy as f32 * ts);
        let t = if span > 1e-4 { (g.cur_q[a] - lo) / span } else { 0.5 };
        let chosen = a == g.cur_action;
        let mut col = q_tint(t);
        col.a = if chosen { 0.95 } else { 0.5 };
        let len = ts * 0.27;
        let th = (ts * 0.045).max(2.0) * if chosen { 1.5 } else { 1.0 };
        let tail = c - dv * len;
        let tip = c + dv * len;
        draw_line(tail.x, tail.y, tip.x - dv.x * ts * 0.1, tip.y - dv.y * ts * 0.1, th, col);
        let perp = Vec2::new(-dv.y, dv.x);
        let hs = ts * 0.13;
        draw_triangle(tip, tip - dv * hs * 1.7 + perp * hs, tip - dv * hs * 1.7 - perp * hs, col);
        if chosen {
            draw_circle_lines(c.x, c.y, ts * 0.42, 2.0, Color::new(1.0, 0.9, 0.4, 0.8));
        }
        if zoom >= 2.4 {
            let txt = format!("{:+.1}", g.cur_q[a]);
            let tw = measure_text(&txt, None, 13, 1.0).width;
            shadow_text(&txt, c.x - tw / 2.0, c.y + ts * 0.40, 13.0, Color::new(col.r, col.g, col.b, if chosen { 1.0 } else { 0.7 }));
        }
    }
    // speech bubble above the hero
    let name = Game::action_name(g.cur_action);
    let l1 = format!("{}   Q {:+.2}", name, g.cur_q[g.cur_action]);
    let l2 = if g.cur_why.is_empty() { "...".to_string() } else { g.cur_why.clone() };
    let w1 = measure_text(&l1, None, 17, 1.0).width;
    let w2 = measure_text(&l2, None, 14, 1.0).width;
    let bw = w1.max(w2) + 22.0;
    let bh = 46.0;
    let bx = (hcx - bw / 2.0).clamp(6.0, vw - bw - 6.0);
    let by = (hcy - ts * 1.58 - bh).max(70.0);
    let bg = Color::new(0.97, 0.94, 0.84, 0.95);
    draw_rectangle(bx, by, bw, bh, bg);
    draw_rectangle_lines(bx, by, bw, bh, 2.0, Color::new(0.45, 0.32, 0.1, 1.0));
    let tx = hcx.clamp(bx + 14.0, bx + bw - 14.0);
    let ty = by + bh;
    draw_triangle(Vec2::new(tx - 9.0, ty - 1.0), Vec2::new(tx + 9.0, ty - 1.0), Vec2::new(hcx.clamp(tx - 20.0, tx + 20.0), (hcy - ts * 1.5).max(ty + 6.0)), bg);
    let ink = Color::new(0.15, 0.1, 0.04, 1.0);
    draw_text(&l1, bx + 11.0, by + 19.0, 17.0, ink);
    let wcol = if l2.starts_with("low HP") { Color::new(0.65, 0.1, 0.1, 1.0) } else { Color::new(0.28, 0.2, 0.08, 1.0) };
    draw_text(&l2, bx + 11.0, by + 38.0, 14.0, wcol);
}

fn draw_actor_labels(g: &Game, zoom: f32) {
    // names + HP bars for monsters at close zoom (screen-space, fixed size)
    if zoom < 1.8 {
        return;
    }
    let o = view_origin(g, zoom);
    let (vw, vh) = Game::view_size();
    let ts = TILE * zoom;
    for (i, m) in g.world.monsters.iter().enumerate() {
        if m.hp <= 0 || m.x < 0 || m.y < 0 {
            continue;
        }
        let mi = g.world.map.idx(m.x as usize, m.y as usize);
        if !g.world.visible[mi] {
            continue;
        }
        let p = g.mon_anim.get(i).map(|a| a.pos).unwrap_or(Vec2::new(m.x as f32, m.y as f32));
        let cx = (p.x * TILE + TILE / 2.0) * zoom + o.x;
        let size = if m.is_boss { 1.45 } else { 1.0 };
        let top = (p.y * TILE + TILE / 2.0) * zoom + o.y - ts * size * 0.5;
        if cx < -50.0 || cx > vw + 50.0 || top < -50.0 || top > vh + 50.0 {
            continue;
        }
        let bw = (ts * 0.95 * size).max(46.0);
        let by = top - 9.0;
        draw_rectangle(cx - bw / 2.0, by, bw, 6.0, Color::new(0.08, 0.03, 0.03, 0.92));
        let f = (m.hp as f32 / m.max_hp as f32).clamp(0.0, 1.0);
        let hc = if f > 0.5 { Color::new(0.35, 0.8, 0.3, 1.0) } else if f > 0.25 { Color::new(0.9, 0.7, 0.2, 1.0) } else { Color::new(0.9, 0.2, 0.2, 1.0) };
        draw_rectangle(cx - bw / 2.0 + 1.0, by + 1.0, (bw - 2.0) * f, 4.0, hc);
        draw_rectangle_lines(cx - bw / 2.0, by, bw, 6.0, 1.0, Color::new(0.7, 0.6, 0.3, 0.9));
        let label = format!("{}  {}/{}", m.name, m.hp, m.max_hp);
        let tw = measure_text(&label, None, 14, 1.0).width;
        let col = if m.is_boss { Color::new(1.0, 0.45, 0.4, 1.0) } else { Color::new(0.95, 0.9, 0.75, 1.0) };
        shadow_text(&label, cx - tw / 2.0, by - 4.0, 14.0, col);
    }
}

fn draw_watch_hud(g: &Game, vw: f32, vh: f32) {
    // bottom strip of the viewport
    let y = vh - 24.0;
    draw_rectangle(0.0, y - 2.0, vw, 26.0, Color::new(0.02, 0.02, 0.03, 0.82));
    let mut left = String::new();
    if let Some(tr) = &g.trainer {
        left = format!("training: episode {}, {:.0} eps/s", tr.episodes(), tr.eps_per_sec());
        shadow_text(&left, 10.0, y + 15.0, 15.0, Color::new(0.45, 1.0, 0.55, 1.0));
    }
    let lw = measure_text(&left, None, 15, 1.0).width;
    if let Some(sc) = &g.showcase {
        let t = format!("showcase #{} (policy from ep {}, turn {})", sc.episode_no, sc.snap.episodes, sc.turns);
        shadow_text(&t, 10.0 + lw + 18.0, y + 15.0, 14.0, Color::new(0.85, 0.82, 0.7, 1.0));
    }
    let mode = if g.watch { "WATCH" } else { "CLASSIC" };
    let cam = if g.follow { "follow" } else { "free cam (F: follow)" };
    let r = format!("{}  {}  zoom {:.1}x  {}", mode, g.speed.label(), g.zoom, cam);
    let rw = measure_text(&r, None, 14, 1.0).width;
    shadow_text(&r, vw - rw - 10.0, y + 15.0, 14.0, AMBER);
}

fn draw_recap(g: &Game, vw: f32, vh: f32) {
    let rc = match &g.recap {
        Some(r) if g.sc_pause > 0.0 && g.sc_pause < 4.4 => r,
        _ => return,
    };
    let a = ((4.4 - g.sc_pause) / 0.4).clamp(0.0, 1.0);
    let w = 320.0;
    let h = 196.0;
    let x = vw / 2.0 - w / 2.0;
    let y = vh / 2.0 - h / 2.0 - 10.0;
    draw_rectangle(x, y, w, h, Color::new(0.06, 0.05, 0.09, 0.93 * a));
    draw_rectangle_lines(x, y, w, h, 2.5, Color::new(0.83, 0.69, 0.22, a));
    let title = format!("EPISODE RECAP  (showcase #{})", rc.episode);
    draw_text(&title, x + 14.0, y + 26.0, 19.0, Color::new(0.83, 0.69, 0.22, a));
    let res = if rc.won { "VICTORY" } else { "DEFEAT" };
    draw_text(res, x + 14.0, y + 52.0, 22.0, if rc.won { Color::new(0.4, 1.0, 0.5, a) } else { Color::new(1.0, 0.4, 0.35, a) });
    let lines = [
        format!("floors reached: {}", rc.floor),
        format!("monsters slain: {}", rc.kills),
        format!("keys found: {}", rc.keys),
        format!("turns: {}   level {}   gold {}", rc.turns, rc.level, rc.gold),
        format!("{}", rc.cause),
    ];
    for (i, l) in lines.iter().enumerate() {
        draw_text(l, x + 14.0, y + 78.0 + i as f32 * 20.0, 16.0, Color::new(0.92, 0.88, 0.78, a));
    }
    let nxt = format!("next showcase in {:.0}s, with the newest Q-table", g.sc_pause.ceil());
    draw_text(&nxt, x + 14.0, y + h - 10.0, 13.0, Color::new(0.6, 0.6, 0.7, a));
}

fn draw_hint(g: &Game, vw: f32, vh: f32) {
    if g.hint_t <= 0.0 {
        return;
    }
    let a = (g.hint_t / 1.5).clamp(0.0, 1.0);
    let lines = [
        "WATCH MODE   (training continues in the background)",
        "wheel / + - : zoom     Z or double-click : close-up     drag : pan     F / Home : follow hero",
        "6 / 7 / 2 : 0.25x / 0.5x / 1x      [ ] : slower / faster      Space : pause",
        "T : thoughts      W : watch on/off      H : heatmap",
    ];
    let w = 640.0f32.min(vw - 20.0);
    let h = 20.0 + lines.len() as f32 * 21.0;
    let x = vw / 2.0 - w / 2.0;
    let y = vh - 24.0 - h - 12.0;
    draw_rectangle(x, y, w, h, Color::new(0.03, 0.03, 0.05, 0.86 * a));
    draw_rectangle_lines(x, y, w, h, 1.5, Color::new(0.83, 0.69, 0.22, a));
    for (i, l) in lines.iter().enumerate() {
        let (fs, col) = if i == 0 { (17.0, AMBER) } else { (14.5, PARCH) };
        let tw = measure_text(l, None, fs as u16, 1.0).width;
        draw_text(l, x + w / 2.0 - tw / 2.0, y + 24.0 + i as f32 * 21.0, fs, Color::new(col.r, col.g, col.b, a));
    }
}

impl Game {
    /// QA helper: fast-forward the showcase to a fight ("combat") or to the end of an episode ("recap").
    pub fn seek(&mut self, what: &str) {
        let keep = self.speed;
        self.speed = Speed::Max;
        for _ in 0..40 {
            for _ in 0..4000 {
                if self.showcase.as_ref().map(|s| s.done).unwrap_or(true) {
                    break;
                }
                let near = self.world.monsters.iter().any(|m| {
                    m.hp > 0 && m.x >= 0 && {
                        let d = (m.x - self.world.hero.x).abs().max((m.y - self.world.hero.y).abs());
                        d <= 2 && self.world.visible[self.world.map.idx(m.x as usize, m.y as usize)]
                    }
                });
                if what == "combat" && near {
                    break;
                }
                self.showcase_turn();
            }
            let done = self.showcase.as_ref().map(|s| s.done).unwrap_or(true);
            let near = what == "combat" && !done;
            if what == "recap" && done {
                self.sc_pause = 4.2;
                break;
            }
            if near && self.world.monsters.iter().any(|m| m.hp > 0 && m.x >= 0 && (m.x - self.world.hero.x).abs().max((m.y - self.world.hero.y).abs()) <= 2) {
                break;
            }
            if done {
                self.start_showcase(None);
            }
        }
        self.speed = keep;
        self.sim_acc = 0.0;
    }
}
