# DungeonMind — PLAN

Stack: Rust + macroquad 0.4. Engine-agnostic game logic in src/lib.rs modules
(rng, fov, dungeon, entities, qlearn, sim, train); rendering in src/render.rs
(macroquad only). `--train N --headless` runs the full loop with no window.

Steps (verified in order):
1. Scaffold + macroquad toolchain check (arm64 + x64). [done-check pending]
2. Core utils: seeded RNG, FOV shadowcasting. Unit tests.
3. Q-learning: state encode (compact u64), update rule, save/load binary. Unit tests.
4. Dungeon gen: rooms/corridors/caves, doors, tiered colored keys/doors
   (blue→green→red reachability proof), traps, secret doors, stairs, fountains,
   chests, torches. Unit tests: connectivity + key-before-door + quests.
5. Entities/combat: 10+ monster kinds w/ D&D d20 combat, hero, inventory,
   quests (fetch/vault, slay, rescue, shrine), bosses every 2nd floor, final boss.
6. Sim: turn engine, action legality masking, reward shaping, events stream
   (for particles/damage numbers/log). Episode = death/win/maxturns.
7. Headless training: >=2000 episodes, stats print (avg reward, depth, eps, win rate).
8. Renderer: procedural textures at startup, camera follow + zoom, fog of war,
   torch flicker lighting, particles, floating damage numbers, screen shake,
   animated tiles, minimap, HUD (HP/XP bars, quest log, combat log).
9. Learning panel: live charts (reward, moving avg, depth, epsilon, Q size,
   win rate), current action + Q-values, heatmap toggle (visit counts / value),
   speed controls (pause/1x/4x/16x/max), step, reset learning, new seed.
10. Sound: procedural blips via macroquad audio, mute toggle (M).
11. Release builds arm64 + x64, copy to Desktop dirs, zip.
12. QA: scheduled-task interactive launch + screenshot, vision review, both arches.
13. README + final report, commit milestones.
