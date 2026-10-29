// gui_ui: input, side panel, charts, HUD, minimap, buttons. (included by gui_draw.rs)

fn button(x: f32, y: f32, w: f32, h: f32, label: &str, active: bool, font: f32) -> bool {
    let mouse = mouse_position();
    let over = mouse.0 >= x && mouse.0 <= x + w && mouse.1 >= y && mouse.1 <= y + h;
    let bg = if active {
        Color::new(0.32, 0.25, 0.12, 1.0)
    } else if over {
        Color::new(0.22, 0.18, 0.1, 1.0)
    } else {
        Color::new(0.12, 0.1, 0.14, 1.0)
    };
    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.6, 0.5, 0.25, 0.7));
    let tw = measure_text(label, None, font as u16, 1.0).width;
    draw_text(
        label,
        x + w / 2.0 - tw / 2.0,
        y + h / 2.0 + font / 3.0,
        font,
        if active { AMBER } else { PARCH },
    );
    over && is_mouse_button_released(MouseButton::Left)
}

fn handle_input(g: &mut Game) {
    if is_key_pressed(KeyCode::Space) {
        if g.speed == Speed::Paused {
            let sp = g.last_speed;
            g.set_speed(sp);
        } else {
            g.speed = Speed::Paused;
        }
    }
    if is_key_pressed(KeyCode::Tab) {
        g.speed = Speed::Paused;
        g.step_once = true;
    }
    for (k, sp) in [
        (KeyCode::Key1, Speed::Paused),
        (KeyCode::Key2, Speed::X1),
        (KeyCode::Key3, Speed::X4),
        (KeyCode::Key4, Speed::X16),
        (KeyCode::Key5, Speed::Max),
        (KeyCode::Key6, Speed::Q25),
        (KeyCode::Key7, Speed::Q50),
    ] {
        if is_key_pressed(k) {
            g.set_speed(sp);
        }
    }
    if is_key_pressed(KeyCode::LeftBracket) {
        g.speed_step(-1);
    }
    if is_key_pressed(KeyCode::RightBracket) {
        g.speed_step(1);
    }
    if is_key_pressed(KeyCode::H) {
        g.heatmap = (g.heatmap + 1) % 3;
    }
    if is_key_pressed(KeyCode::W) {
        let on = !g.watch;
        g.set_watch(on);
        if on {
            g.zoom_target = WATCH_ZOOM;
            g.follow = true;
            if g.speed.mult() > 1.0 {
                g.set_speed(Speed::Q50);
            }
        }
    }
    if is_key_pressed(KeyCode::T) {
        g.thought_on = !g.thought_on;
    }
    if is_key_pressed(KeyCode::R) {
        g.reset_learning();
        if !g.watch {
            let seed = g.cur_seed;
            g.world = World::new(seed);
            g.drain_events();
        }
    }
    if is_key_pressed(KeyCode::N) {
        g.new_seed();
    }
    if is_key_pressed(KeyCode::D) {
        g.seed_fixed = !g.seed_fixed;
        g.log.push(format!(
            "Seed mode: {}.",
            if g.seed_fixed { "FIXED seed" } else { "NEW seed each episode" }
        ));
        g.trim_log();
    }
    if is_key_pressed(KeyCode::S) {
        g.save_now();
        g.log.push("Q-table saved.".into());
        g.trim_log();
    }
}

fn chart(x: f32, y: f32, w: f32, h: f32, title: &str, data: &[f32], col: Color, win: usize) {
    draw_rectangle(x, y, w, h, Color::new(0.08, 0.07, 0.1, 1.0));
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.35, 0.3, 0.2, 0.6));
    draw_text(title, x + 4.0, y + 11.0, 12.0, PARCH);
    let view: Vec<f32> = if win == 0 {
        data.to_vec()
    } else {
        data.iter().rev().take(win).rev().copied().collect()
    };
    if view.len() < 2 {
        return;
    }
    let lo = view.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = view.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let rng = (hi - lo).max(1e-3);
    let n = view.len();
    for i in 1..n {
        let x0 = x + (i - 1) as f32 / (n - 1) as f32 * (w - 8.0) + 4.0;
        let x1 = x + i as f32 / (n - 1) as f32 * (w - 8.0) + 4.0;
        let y0 = y + h - 4.0 - (view[i - 1] - lo) / rng * (h - 20.0);
        let y1 = y + h - 4.0 - (view[i] - lo) / rng * (h - 20.0);
        draw_line(x0, y0, x1, y1, 1.5, col);
    }
    let lab = format!("{:.0} .. {:.0}", lo, hi);
    draw_text(&lab, x + w - 60.0, y + 11.0, 10.0, Color::new(0.6, 0.58, 0.5, 1.0));
}

fn draw_panel(g: &mut Game, sw: f32, sh: f32, fps: f32) {
    let px = sw - PANEL_W;
    draw_rectangle(px, 0.0, PANEL_W, sh, Color::new(0.07, 0.06, 0.09, 1.0));
    draw_rectangle(px, 0.0, 3.0, sh, Color::new(0.6, 0.5, 0.25, 0.9));
    let mut y = 10.0;
    let cx = px + 12.0;
    draw_text("THE MIND OF THE HERO", cx, y + 14.0, 17.0, AMBER);
    y += 26.0;
    let info = format!(
        "ep {}   eps {:.2}   q-states {}   {:.0} fps",
        g.episodes_total,
        g.eps_now(),
        g.qsize_now(),
        fps
    );
    draw_text(&info, cx, y + 10.0, 12.0, PARCH);
    y += 18.0;
    let cur = format!(
        "now: floor {}  hp {}/{}  turn {}  r {:.0}",
        g.world.floor_no + 1,
        g.world.hero.hp.max(0),
        g.world.hero.max_hp,
        g.world.turn,
        g.ep_reward
    );
    draw_text(&cur, cx, y + 10.0, 12.0, Color::new(0.75, 0.75, 0.85, 1.0));
    y += 20.0;
    // charts
    let cw = PANEL_W - 24.0;
    let ch = 52.0;
    chart(cx, y, cw, ch, "reward / ep  (MA100 gold)", &g.hist.rew, Color::new(0.4, 0.7, 1.0, 1.0), 300);
    chart(cx + 0.0, y + 1.0, cw, ch - 2.0, "", &g.hist.ma, AMBER, 300);
    y += ch + 4.0;
    chart(cx, y, cw, ch, "deepest floor (per ep)", &g.hist.floor, Color::new(0.5, 0.9, 0.5, 1.0), 300);
    y += ch + 4.0;
    let wr: Vec<f32> = {
        let mut v = Vec::new();
        for i in 0..g.hist.win.len() {
            let a = i.saturating_sub(99);
            let s: f32 = g.hist.win[a..=i].iter().sum();
            v.push(s / (i - a + 1) as f32);
        }
        v
    };
    chart(cx, y, cw, ch, "epsilon", &g.hist.eps, Color::new(1.0, 0.5, 0.5, 1.0), 0);
    y += ch + 4.0;
    chart(cx, y, cw, ch, "win rate (100 ep)", &wr, Color::new(1.0, 0.85, 0.3, 1.0), 0);
    y += ch + 6.0;
    // current action + Q-values
    draw_text("Q-VALUES OF CURRENT STATE", cx, y + 10.0, 12.0, AMBER);
    y += 16.0;
    let lo = g.cur_q.iter().cloned().fold(f32::INFINITY, f32::min).min(0.0);
    let hi = g.cur_q.iter().cloned().fold(f32::NEG_INFINITY, f32::max).max(0.0);
    let rng = (hi - lo).max(1e-3);
    for a in 0..N_ACTIONS {
        let by = y + a as f32 * 10.0;
        let name = Game::action_name(a);
        draw_text(name, cx, by + 8.0, 10.0, if g.cur_legal[a] { PARCH } else { Color::new(0.4, 0.38, 0.42, 1.0) });
        let zx = cx + 78.0;
        let zw = cw - 78.0 - 44.0;
        draw_rectangle(zx, by + 1.0, zw, 7.0, Color::new(0.1, 0.09, 0.13, 1.0));
        let frac = (g.cur_q[a] - lo) / rng;
        let zero = (0.0 - lo) / rng;
        let x0 = zx + zero.min(frac) * zw;
        let x1 = zx + zero.max(frac) * zw;
        let col = if a == g.cur_action {
            AMBER
        } else if g.cur_legal[a] {
            Color::new(0.3, 0.7, 0.9, 1.0)
        } else {
            Color::new(0.2, 0.2, 0.25, 1.0)
        };
        draw_rectangle(x0, by + 1.0, (x1 - x0).max(1.0), 7.0, col);
        draw_text(&format!("{:.1}", g.cur_q[a]), zx + zw + 4.0, by + 8.0, 9.0, Color::new(0.7, 0.7, 0.75, 1.0));
    }
    y += N_ACTIONS as f32 * 10.0 + 6.0;
    // quests
    draw_text("QUEST LOG", cx, y + 10.0, 12.0, AMBER);
    y += 16.0;
    if g.world.quests.quests.is_empty() {
        draw_text("(none)", cx, y + 9.0, 11.0, Color::new(0.5, 0.5, 0.55, 1.0));
        y += 14.0;
    }
    for (q, done) in g.world.quests.quests.iter() {
        let mark = if *done { "[done] " } else { "[  +  ] " };
        let col = if *done {
            Color::new(0.45, 0.8, 0.45, 1.0)
        } else {
            PARCH
        };
        draw_text(&format!("{}{}", mark, q.label()), cx, y + 9.0, 11.0, col);
        y += 13.0;
    }
    y += 4.0;
    // speed buttons + toggles
    let bw = (cw - 12.0) / 7.0;
    let gy = y + 2.0;
    for (i, sp) in Speed::ALL.iter().enumerate() {
        if button(cx + i as f32 * (bw + 2.0), gy, bw, 22.0, sp.label(), g.speed == *sp, 10.0) {
            g.set_speed(*sp);
        }
    }
    let by2 = gy + 26.0;
    let third = (cw - 4.0) / 3.0;
    if button(cx, by2, third, 22.0, "STEP", false, 10.0) {
        g.speed = Speed::Paused;
        g.step_once = true;
    }
    if button(cx + third + 2.0, by2, third, 22.0, "RESET LEARN", false, 10.0) {
        g.reset_learning();
        let s = g.cur_seed;
        g.world = World::new(s);
        g.drain_events();
    }
    if button(cx + 2.0 * (third + 2.0), by2, third, 22.0, "NEW SEED", false, 10.0) {
        g.new_seed();
    }
    let by3 = by2 + 26.0;
    let w3 = (cw - 4.0) / 3.0;
    if button(cx, by3, w3, 22.0, if g.watch { "WATCH: ON" } else { "WATCH: off" }, g.watch, 10.0) {
        let on = !g.watch;
        g.set_watch(on);
        if on {
            g.zoom_target = WATCH_ZOOM;
            g.follow = true;
        }
    }
    let th = if g.thought_on { "THOUGHTS: ON" } else { "THOUGHTS: off" };
    if button(cx + w3 + 2.0, by3, w3, 22.0, th, g.thought_on, 10.0) {
        g.thought_on = !g.thought_on;
    }
    if button(cx + 2.0 * (w3 + 2.0), by3, w3, 22.0, "CLOSE-UP", false, 10.0) {
        g.zoom_target = WATCH_ZOOM;
        g.follow = true;
    }
    let kc = Color::new(0.55, 0.53, 0.6, 1.0);
    draw_text("space pause · 1-7 speed · [ ] slower/faster · Tab step · W watch · T thoughts", cx, by3 + 36.0, 9.5, kc);
    draw_text("wheel/+- zoom · Z close-up · drag pan · F follow · R reset · N seed · D fixed-seed · H heat · S save", cx, by3 + 47.0, 9.5, kc);
}

fn draw_hud(g: &Game, sw: f32, sh: f32, vw: f32, _vh: f32) {
    let y = sh - HUD_H;
    draw_rectangle(0.0, y, sw, HUD_H, Color::new(0.06, 0.05, 0.08, 1.0));
    draw_rectangle(0.0, y, sw, 2.0, Color::new(0.6, 0.5, 0.25, 0.9));
    let h = &g.world.hero;
    // HP bar
    let bx = 14.0;
    let bw = 300.0;
    bar(bx, y + 12.0, bw, 16.0, h.hp as f32 / h.max_hp as f32, Color::new(0.75, 0.12, 0.15, 1.0), Color::new(0.25, 0.05, 0.06, 1.0));
    draw_text(
        &format!("HP {}/{}  AC {}  Lv {}", h.hp.max(0), h.max_hp, h.ac, h.level),
        bx + 6.0,
        y + 24.0,
        12.0,
        Color::new(1.0, 0.95, 0.9, 1.0),
    );
    // XP bar
    bar(bx, y + 36.0, bw, 12.0, h.xp as f32 / h.xp_next() as f32, Color::new(0.25, 0.5, 0.85, 1.0), Color::new(0.08, 0.12, 0.2, 1.0));
    draw_text(&format!("XP {}/{}", h.xp, h.xp_next()), bx + 6.0, y + 45.5, 10.0, Color::new(0.85, 0.9, 1.0, 1.0));
    // inventory
    let mut ix = bx + bw + 24.0;
    draw_text(&format!("{} gold", h.gold), ix, y + 24.0, 13.0, AMBER);
    ix += 90.0;
    draw_circle(ix, y + 19.0, 7.0, Color::new(0.85, 0.15, 0.2, 1.0));
    draw_text(&format!("x{}", h.potions), ix + 10.0, y + 24.0, 13.0, PARCH);
    ix += 55.0;
    for t in 0..3 {
        let n = h.keys[t];
        draw_rectangle(ix, y + 12.0, 14.0, 14.0, if n > 0 { TIER_COLORS[t] } else { Color::new(0.15, 0.14, 0.18, 1.0) });
        draw_rectangle_lines(ix, y + 12.0, 14.0, 14.0, 1.0, Color::new(0.6, 0.5, 0.25, 0.6));
        draw_text(&format!("{}", n), ix + 17.0, y + 24.0, 11.0, PARCH);
        ix += 42.0;
    }
    draw_text(&format!("floor {}/8", g.world.floor_no + 1), ix + 8.0, y + 24.0, 14.0, PARCH);
    draw_text(
        &format!("weapon {}d{}   seed {}", h.dmg.0, h.dmg.1, g.cur_seed % 1_000_000),
        ix + 8.0,
        y + 44.0,
        11.0,
        Color::new(0.7, 0.68, 0.75, 1.0),
    );
    // combat log (3 lines) top-left of viewport
    let ly = 14.0;
    let lines = g.log.len().saturating_sub(0);
    let start = g.log.len().saturating_sub(4);
    for (i, l) in g.log[start..lines].iter().enumerate() {
        let amber = l.starts_with('—');
        let t: String = l.chars().map(|c| if c == '—' { '-' } else { c }).take(120).collect();
        let tw = measure_text(&t, None, 12, 1.0).width;
        draw_rectangle(8.0, ly + i as f32 * 15.0 - 11.0, tw + 10.0, 15.0, Color::new(0.02, 0.02, 0.03, 0.78));
        draw_text(&t, 13.0, ly + i as f32 * 15.0, 12.0, if amber { AMBER } else { PARCH });
    }
    let _ = vw;
}

fn bar(x: f32, y: f32, w: f32, h: f32, frac: f32, fg: Color, bgr: Color) {
    draw_rectangle(x, y, w, h, bgr);
    draw_rectangle(x, y, w * frac.clamp(0.0, 1.0), h, fg);
    draw_rectangle_lines(x, y, w, h, 1.0, Color::new(0.6, 0.5, 0.25, 0.8));
}

fn draw_overlay(g: &Game, _art: &Art, vw: f32, _vh: f32) {
    // minimap
    let mw = 168.0;
    let mh = mw * (g.world.map.h as f32 / g.world.map.w as f32);
    let mx = vw - mw - 12.0;
    let my = 12.0;
    draw_rectangle(mx - 3.0, my - 3.0, mw + 6.0, mh + 6.0, Color::new(0.03, 0.025, 0.04, 0.9));
    draw_rectangle_lines(mx - 3.0, my - 3.0, mw + 6.0, mh + 6.0, 1.0, Color::new(0.6, 0.5, 0.25, 0.8));
    let sx = mw / g.world.map.w as f32;
    let sy = mh / g.world.map.h as f32;
    let mut img = Image::gen_image_color(g.world.map.w as u16, g.world.map.h as u16, Color::new(0.0, 0.0, 0.0, 0.0));
    for yy in 0..g.world.map.h {
        for xx in 0..g.world.map.w {
            let i = g.world.map.idx(xx, yy);
            let c = if g.world.visible[i] {
                Color::new(0.85, 0.8, 0.6, 1.0)
            } else if g.world.explored[i] {
                match g.world.map.cells[i].t {
                    Tile::Wall | Tile::SecretDoor | Tile::Torch => Color::new(0.0, 0.0, 0.0, 0.0),
                    _ => Color::new(0.35, 0.33, 0.42, 1.0),
                }
            } else {
                Color::new(0.0, 0.0, 0.0, 0.0)
            };
            img.set_pixel(xx as u32, yy as u32, c);
        }
    }
    let tex = Texture2D::from_image(&img);
    draw_texture_ex(&tex, mx, my, CWHITE, DrawTextureParams { dest_size: Some(Vec2::new(mw, mh)), ..Default::default() });
    // hero dot
    draw_circle(mx + g.world.hero.x as f32 * sx + sx / 2.0, my + g.world.hero.y as f32 * sy + sy / 2.0, 2.5, Color::new(0.2, 1.0, 0.4, 1.0));
    for m in g.world.monsters.iter().filter(|m| m.hp > 0 && m.is_boss) {
        let i = g.world.map.idx(m.x as usize, m.y as usize);
        if g.world.explored[i] {
            draw_circle(mx + m.x as f32 * sx, my + m.y as f32 * sy, 2.0, Color::new(1.0, 0.2, 0.2, 1.0));
        }
    }
    drop(tex);
    let _ = (sx, sy);
}

fn draw_boss_bar(g: &Game, vw: f32) {
    let boss = g.world.monsters.iter().find(|m| m.is_boss && m.hp > 0 && {
        let i = g.world.map.idx(m.x.max(0) as usize, m.y.max(0) as usize);
        (m.x >= 0 && m.y >= 0) && g.world.visible.get(i).copied().unwrap_or(false)
    });
    if let Some(b) = boss {
        let bw = (vw * 0.6).min(700.0);
        let bx = vw / 2.0 - bw / 2.0;
        let by = 54.0;
        draw_text(&b.name.to_uppercase(), bx, by - 6.0, 15.0, Color::new(1.0, 0.35, 0.3, 1.0));
        let frac = b.hp as f32 / b.max_hp as f32;
        let phase = if frac > 0.66 { 1 } else if frac > 0.33 { 2 } else { 3 };
        let col = match phase {
            1 => Color::new(0.8, 0.2, 0.2, 1.0),
            2 => Color::new(0.9, 0.5, 0.1, 1.0),
            _ => Color::new(1.0, 0.85, 0.2, 1.0),
        };
        bar(bx, by, bw, 18.0, frac, col, Color::new(0.15, 0.05, 0.05, 1.0));
        draw_text(
            &format!("{} - phase {}", b.hp.max(0), phase),
            bx + bw / 2.0 - 40.0,
            by + 13.5,
            11.0,
            Color::new(1.0, 0.97, 0.9, 1.0),
        );
    }
}
