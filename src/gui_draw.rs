// gui_draw: window setup, main loop, world rendering. (included by gui.rs)

pub fn window_conf() -> Conf {
    Conf {
        window_title: "DungeonMind — a hero that learns".to_string(),
        window_width: 1240,
        window_height: 700,
        window_resizable: true,
        ..Default::default()
    }
}

fn bg_color() -> Color {
    Color::new(0.043, 0.035, 0.055, 1.0)
}

#[macroquad::main(window_conf)]
pub async fn main() {
    let art = gen_art();
    let path = train::qtable_path();
    let mut agent = QAgent::new();
    let loaded = agent.load(&path);
    let opts = parse_opts();
    let mut g = Game::fresh(opts.seed, opts.fixed, agent);
    g.thought_on = opts.thoughts;
    g.speed = opts.speed.unwrap_or(if opts.watch { Speed::Q50 } else { Speed::X4 });
    g.last_speed = if g.speed == Speed::Paused { Speed::Q50 } else { g.speed };
    g.zoom_target = match opts.zoom {
        Some(z) if z <= 0.0 => g.fit_zoom(),
        Some(z) => z.min(MAX_ZOOM),
        None => {
            if opts.watch {
                WATCH_ZOOM
            } else {
                1.15
            }
        }
    };
    g.zoom = g.zoom_target;
    g.hint_t = if opts.hint { 11.0 } else { 0.0 };
    if loaded {
        g.log.push(format!(
            "Loaded Q-table: {} states, {} episodes (eps {:.2}).",
            g.agent.q.len(),
            g.agent.episodes,
            g.agent.eps
        ));
    }
    if opts.watch {
        g.set_watch(true);
        if let Some(w) = &opts.seek {
            g.seek(w);
            g.sync_anims(1.0);
            g.hero_anim = Anim::at(g.world.hero.x, g.world.hero.y);
            g.cam = Vec2::ZERO;
        }
    }
    let mut fps_disp = 60.0;
    // Pre-warm the font atlas (all sizes/glyphs we use) for two frames, so it
    // never grows mid-run: growth while playing corrupted glyphs on software GL.
    for _ in 0..2 {
        clear_background(bg_color());
        warm_fonts();
        next_frame().await;
    }
    loop {
        let dt = get_frame_time().min(0.05);
        g.time += dt;
        fps_disp = fps_disp * 0.95 + (1.0 / dt.max(0.001)) * 0.05;
        handle_input(&mut g);
        g.camera_input(dt);
        if g.time - g.last_save > 5.0 {
            g.last_save = g.time;
            g.save_now();
        }
        // brief slow-motion moments (boss hit, hero death) scale game time only
        let sdt = if g.slowmo > 0.0 && !g.speed.is_fast() { dt * 0.3 } else { dt };
        g.slowmo = (g.slowmo - dt).max(0.0);
        g.hint_t = (g.hint_t - dt).max(0.0);
        g.pump_trainer();
        simulate(&mut g, sdt);
        g.sync_anims(sdt);
        update_fx(&mut g, sdt);
        update_camera(&mut g, dt);
        render(&mut g, &art, fps_disp);
        next_frame().await;
    }
}

fn warm_fonts() {
    // only the sizes the UI really uses, ASCII + middle dot: keeps the atlas small
    let glyphs: String = (32u8..127).map(|c| c as char).chain(std::iter::once('\u{b7}')).collect();
    let mut sizes: Vec<u16> = (9u16..=22).collect();
    sizes.extend([24, 28, 32, 36, 40, 44, 48, 52]);
    for size in sizes {
        draw_text(&glyphs, 2.0, 20.0, size as f32, Color::new(0.0, 0.0, 0.0, 0.01));
    }
}

fn update_camera(g: &mut Game, dt: f32) {
    let hero_w = Vec2::new(g.hero_anim.pos.x * TILE + TILE / 2.0, g.hero_anim.pos.y * TILE + TILE / 2.0);
    if g.cam.x.abs() < 1.0 && g.cam.y.abs() < 1.0 {
        g.cam = hero_w;
    }
    if g.follow {
        let k = 1.0 - (-7.0 * dt).exp();
        g.cam += (hero_w - g.cam) * k;
    }
    let z = g.zoom;
    g.clamp_cam(z);
}

fn simulate(g: &mut Game, dt: f32) {
    if g.watch {
        simulate_watch(g, dt);
        return;
    }
    match g.speed {
        Speed::Paused => {
            if g.step_once {
                g.step_once = false;
                g.run_turn();
            }
        }
        // classic (learning-on-screen) fast speeds keep the per-frame batches
        Speed::X4 => {
            for _ in 0..4 {
                g.run_turn();
            }
        }
        Speed::X16 => {
            for _ in 0..16 {
                g.run_turn();
            }
        }
        Speed::Max => {
            let t0 = std::time::Instant::now();
            while t0.elapsed().as_micros() < 6000 {
                g.run_turn();
            }
        }
        sp => {
            // 0.25x / 0.5x / 1x: real-time pacing
            let n = watch::turns_due(&mut g.sim_acc, dt, sp.mult());
            for _ in 0..n {
                g.run_turn();
            }
        }
    }
}

fn simulate_watch(g: &mut Game, dt: f32) {
    if g.showcase.as_ref().map(|s| s.done).unwrap_or(false) {
        g.sc_pause -= dt;
        if g.sc_pause <= 0.0 || g.step_once {
            g.step_once = false;
            g.start_showcase(None);
        }
        return;
    }
    if g.speed == Speed::Paused {
        if g.step_once {
            g.step_once = false;
            g.showcase_turn();
        }
        return;
    }
    let n = watch::turns_due(&mut g.sim_acc, dt, g.speed.mult());
    for _ in 0..n {
        g.showcase_turn();
        if g.showcase.as_ref().map(|s| s.done).unwrap_or(true) {
            break;
        }
    }
}

fn update_fx(g: &mut Game, dt: f32) {
    g.shake *= (-8.0 * dt).exp();
    if g.shake < 0.05 {
        g.shake = 0.0;
    }
    for p in g.parts.iter_mut() {
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.vy += p.grav * dt;
        p.life -= dt;
    }
    g.parts.retain(|p| p.life > 0.0);
    for f in g.floats.iter_mut() {
        f.y += f.vy * dt;
        f.vy *= (-2.0 * dt).exp();
        f.life -= dt;
    }
    g.floats.retain(|f| f.life > 0.0);
}

fn tile_tex<'a>(art: &'a Art, w: &World, x: usize, y: usize) -> &'a Texture2D {
    let c = &w.map.cells[w.map.idx(x, y)];
    let i = w.map.idx(x, y);
    match c.t {
        Tile::Wall | Tile::SecretDoor | Tile::Torch => &art.wall,
        Tile::Floor | Tile::Captive => &art.floor,
        Tile::Corridor => &art.corridor,
        Tile::Cave => &art.cave,
        Tile::Water => &art.water,
        Tile::Lava => &art.lava,
        Tile::StairsDown => &art.stairs,
        Tile::Fountain => {
            if w.used_fountains.contains(&i) {
                &art.fountain_used
            } else {
                &art.fountain
            }
        }
        Tile::Chest => {
            if w.opened_chests.contains(&i) {
                &art.chest_open
            } else {
                &art.chest
            }
        }
        Tile::Trap => {
            if w.trap_revealed(x, y) {
                &art.trap
            } else {
                &art.floor
            }
        }
        Tile::Shrine => &art.shrine,
        Tile::Door => match c.locked {
            0 => &art.doors[0],
            1 => &art.doors[1],
            2 => &art.doors[2],
            _ => &art.door_open,
        },
        Tile::Vault => {
            if c.locked >= 0 {
                &art.vault
            } else {
                &art.door_open
            }
        }
    }
}

fn render(g: &mut Game, art: &Art, fps: f32) {
    clear_background(bg_color());
    let sw = screen_width();
    let sh = screen_height();
    let vw = (sw - PANEL_W).max(320.0);
    let vh = (sh - HUD_H).max(240.0);
    let zoom = g.zoom;
    draw_world(g, art, zoom);
    draw_actor_labels(g, zoom);
    draw_thought(g, zoom);
    draw_dice(g, zoom);
    set_default_camera();
    draw_overlay(g, art, vw, vh);
    draw_panel(g, sw, sh, fps);
    draw_hud(g, sw, sh, vw, vh);
    draw_boss_bar(g, vw);
    draw_watch_hud(g, vw, vh);
    draw_recap(g, vw, vh);
    draw_hint(g, vw, vh);
}

fn draw_world(g: &Game, art: &Art, zoom: f32) {
    // Screen-space projection (no macroquad custom camera: the Camera2D path
    // renders nothing on software GL stacks like Mesa llvmpipe, which is what
    // GPU-less VMs fall back to). world px -> screen px:
    let sw = screen_width();
    let sh = screen_height();
    let vw = (sw - PANEL_W).max(320.0);
    let vh = (sh - HUD_H).max(240.0);
    let org = view_origin(g, zoom);
    let (ox, oy) = (org.x, org.y);
    let sx = |wx: f32| wx * zoom + ox;
    let sy = |wy: f32| wy * zoom + oy;
    let w = &g.world;
    let half_x = vw / 2.0 / zoom;
    let half_y = vh / 2.0 / zoom;
    let x0 = ((g.cam.x - half_x) / TILE).floor().max(0.0) as usize;
    let x1 = ((g.cam.x + half_x) / TILE).ceil().min(w.map.w as f32 - 1.0) as usize;
    let y0 = ((g.cam.y - half_y) / TILE).floor().max(0.0) as usize;
    let y1 = ((g.cam.y + half_y) / TILE).ceil().min(w.map.h as f32 - 1.0) as usize;
    let tsz = TILE * zoom;
    let hr = w.light_radius as f32;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let i = w.map.idx(x, y);
            if !w.explored[i] {
                continue;
            }
            let vis = w.visible[i];
            let (wx, wy) = (sx(x as f32 * TILE), sy(y as f32 * TILE));
            let tex = tile_tex(art, w, x, y);
            let c = &w.map.cells[i];
            // animated liquids & torch glow use time-varying tints
            let mut tint = if vis {
                let d = (((x as i32 - w.hero.x).pow(2) + (y as i32 - w.hero.y).pow(2)) as f32).sqrt();
                let flick = if c.torch { (g.time * 11.0 + x as f32 * 3.1 + y as f32 * 7.7).sin() * 0.06 } else { 0.0 };
                let light = 1.05 - (d / (hr + 2.5)) * 0.5 + flick;
                let l = light.clamp(0.62, 1.2);
                Color::new(l, l * 0.99, l * 1.03, 1.0)
            } else {
                Color::new(0.5, 0.49, 0.6, 0.8) // explored fog
            };
            if c.t == Tile::Water && vis {
                let s = (g.time * 2.2 + x as f32 * 0.8 + y as f32 * 1.2).sin() * 0.08;
                tint = Color::new(tint.r * (0.9 + s), tint.g * (0.95 + s), tint.b * (1.05 + s), tint.a);
            }
            if c.t == Tile::Lava {
                let s = (g.time * 3.0 + (x * 7 + y * 13) as f32 * 0.25).sin() * 0.12;
                tint = Color::new(tint.r + s, tint.g + s * 0.4, tint.b, tint.a);
            }
            draw_texture_ex(tex, wx, wy, tint, DrawTextureParams {
                dest_size: Some(Vec2::new(tsz, tsz)),
                ..Default::default()
            });
            // heatmap
            if g.heatmap == 1 {
                let v = w.visits[i] as f32;
                if v > 0.0 {
                    let a = (v.ln() * 0.18).min(0.55);
                    draw_rectangle(wx, wy, tsz, tsz, Color::new(1.0, 0.45, 0.1, a));
                }
            } else if g.heatmap == 2 {
                let a = if vis_dummy(w, i) { 0.30 } else { 0.08 };
                draw_rectangle(wx, wy, tsz, tsz, Color::new(0.2, 0.5, 1.0, a));
            }
        }
    }
    // torch flames
    for (tx, ty) in w.map.torch_positions.iter() {
        let i = w.map.idx(*tx, *ty);
        if !w.visible[i] && !w.explored[i] {
            continue;
        }
        let fx = sx(*tx as f32 * TILE + TILE / 2.0);
        let fy = sy(*ty as f32 * TILE + 4.0);
        let fl = 1.0 + (g.time * 13.0 + *tx as f32).sin() * 0.18 + (g.time * 23.0 + *ty as f32).sin() * 0.09;
        draw_circle(fx, fy, 3.2 * fl * zoom, Color::new(1.0, 0.62, 0.15, 0.95));
        draw_circle(fx, fy - 1.5 * zoom, 1.6 * fl * zoom, Color::new(1.0, 0.92, 0.5, 0.95));
        draw_circle(fx, fy, 9.0 * TILE * 0.08 * fl * zoom, Color::new(1.0, 0.55, 0.15, 0.06));
    }
    // items
    for (ix, iy, kind) in w.items.iter() {
        let i = w.map.idx(*ix, *iy);
        if !w.visible[i] {
            continue;
        }
        let fx = sx(*ix as f32 * TILE + TILE / 2.0);
        let fy = sy(*iy as f32 * TILE + TILE / 2.0);
        match kind {
            ItemKind::Potion => {
                draw_circle(fx, fy + 2.0 * zoom, 4.0 * zoom, Color::new(0.85, 0.15, 0.2, 1.0));
                draw_rectangle(fx - 1.2 * zoom, fy - 6.0 * zoom, 2.4 * zoom, 4.0 * zoom, Color::new(0.75, 0.85, 0.9, 1.0));
            }
            ItemKind::Gold => {
                draw_circle(fx, fy, 4.0 * zoom, AMBER);
                draw_circle(fx - 1.2 * zoom, fy - 1.2 * zoom, 1.3 * zoom, Color::new(1.0, 1.0, 0.75, 1.0));
            }
            ItemKind::Key(t) => {
                let col = TIER_COLORS[*t];
                draw_circle(fx - 2.5 * zoom, fy, 3.0 * zoom, col);
                draw_rectangle(fx - 1.0 * zoom, fy - 1.0 * zoom, 7.0 * zoom, 2.0 * zoom, col);
                draw_rectangle(fx + 4.0 * zoom, fy, 1.5 * zoom, 3.0 * zoom, col);
            }
        }
    }
    // captive
    if let Some((cx, cy)) = w.quests.captive_spot {
        let i = w.map.idx(cx, cy);
        if w.explored[i] {
            let t = if w.visible[i] { CWHITE } else { Color::new(0.6, 0.6, 0.7, 0.8) };
            draw_texture_ex(&art.captive, sx(cx as f32 * TILE), sy(cy as f32 * TILE), t, DrawTextureParams {
                dest_size: Some(Vec2::new(tsz, tsz)),
                ..Default::default()
            });
        }
    }
    // monsters
    for (mi_idx, m) in w.monsters.iter().enumerate().filter(|(_, m)| m.hp > 0) {
        if m.x < 0 || m.y < 0 {
            continue;
        }
        let mi = w.map.idx(m.x as usize, m.y as usize);
        if !w.visible[mi] {
            continue;
        }
        let ma = g.mon_anim.get(mi_idx).copied().unwrap_or(Anim::at(m.x, m.y));
        let mp = ma.pos + ma.lunge_off();
        let bob = ((g.time * 5.0 + m.x as f32).sin() * 1.2).floor();
        let size = if m.is_boss { TILE * 1.45 } else { TILE };
        let mut t = if m.kind == Kind::Wraith {
            Color::new(0.8, 0.9, 1.0, 0.7)
        } else {
            CWHITE
        };
        if ma.flash > 0.0 {
            t = Color::new(1.0, 1.0 - 0.1 * ma.flash, 1.0 - 0.1 * ma.flash, t.a);
            t = Color::new(t.r * 1.0 + ma.flash, t.g + ma.flash, t.b + ma.flash, t.a);
        }
        draw_texture_ex(
            art.mon_tex(m.kind),
            sx(mp.x * TILE + TILE / 2.0 - size / 2.0),
            sy(mp.y * TILE + TILE / 2.0 - size / 2.0 + bob),
            t,
            DrawTextureParams {
                dest_size: Some(Vec2::new(size * zoom, size * zoom)),
                ..Default::default()
            },
        );
        if m.hp < m.max_hp && zoom < 1.8 {
            let bw = size * 0.9 * zoom;
            let bx = sx(mp.x * TILE + (TILE - size * 0.9) / 2.0);
            let by = sy(mp.y * TILE - 3.0);
            draw_rectangle(bx, by, bw, 2.5 * zoom, Color::new(0.1, 0.05, 0.05, 0.9));
            draw_rectangle(bx, by, bw * (m.hp as f32 / m.max_hp as f32), 2.5 * zoom, Color::new(0.85, 0.2, 0.2, 1.0));
        }
        if (m.is_boss || m.name.contains("the")) && zoom < 1.8 {
            let nm = m.name.clone();
            let fs = qsize(13.0 * zoom) as u16;
            let tw = measure_text(&nm, None, fs, 1.0).width;
            draw_text(
                &nm,
                sx(mp.x * TILE + TILE / 2.0) - tw / 2.0,
                sy(mp.y * TILE - 5.0),
                fs as f32,
                if m.is_boss { Color::new(1.0, 0.4, 0.35, 1.0) } else { Color::new(0.9, 0.8, 0.5, 1.0) },
            );
        }
    }
    // hero
    let hb = ((g.time * 6.0).sin() * 0.8).floor();
    let hp = g.hero_anim.pos + g.hero_anim.lunge_off();
    let fl = g.hero_anim.flash;
    let mut hcol = Color::new(1.0 + fl * 0.0, 1.0 - fl * 0.55, 1.0 - fl * 0.55, 1.0);
    if w.dead {
        hcol = Color::new(0.55, 0.35, 0.35, 0.8);
    }
    draw_texture_ex(
        &art.hero,
        sx(hp.x * TILE),
        sy(hp.y * TILE + hb),
        hcol,
        DrawTextureParams { dest_size: Some(Vec2::new(tsz, tsz)), ..Default::default() },
    );
    // hero light glow
    draw_circle(sx(hp.x * TILE + TILE / 2.0), sy(hp.y * TILE + TILE / 2.0), TILE * 1.1 * zoom, Color::new(1.0, 0.9, 0.6, 0.05));
    // particles
    for p in g.parts.iter() {
        let a = (p.life / p.max).clamp(0.0, 1.0);
        let mut c = p.col;
        c.a = a;
        draw_rectangle(sx(p.x - p.size / 2.0), sy(p.y - p.size / 2.0), p.size * zoom, p.size * zoom, c);
    }
    // floating text
    for f in g.floats.iter() {
        let a = (f.life / 0.6).clamp(0.0, 1.0);
        let mut c = f.col;
        c.a = a;
        let tw = measure_text(&f.txt, None, f.size as u16, 1.0).width;
        let fz = qsize(f.size * zoom.clamp(0.9, 1.7));
        let tw = tw * fz / f.size;
        draw_text(&f.txt, sx(f.x) - tw / 2.0 + 1.5, sy(f.y) + 1.5, fz, Color::new(0.0, 0.0, 0.0, a * 0.8));
        draw_text(&f.txt, sx(f.x) - tw / 2.0, sy(f.y), fz, c);
    }
}

fn vis_dummy(w: &World, i: usize) -> bool {
    w.visible[i]
}
include!("gui_watch.rs");
include!("gui_ui.rs");
