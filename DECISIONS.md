# DungeonMind — DECISIONS LOG

- Stack: Rust + macroquad (pure-Rust miniquad GL backend; single self-contained
  exe per arch; both cargo targets installed; low -j2 builds to fit 8 GB RAM).
- Game logic engine-agnostic in lib.rs; main.rs branches on --headless before
  creating any window (SSH Session 0 can't make windows; GUI QA via scheduled task).
- 8 floors, each 128x80 tiles. Mixed layouts per floor: rooms+corridors or caves.
- Locked-door model: 3 key tiers (blue < green < red). A tier-c key is always
  placed in the region reachable entering only doors of tier < c, guaranteeing
  key-before-door by construction; verified by unit test per generated floor.
- Q-learning: tabular HashMap<u64,[f32;13]> over a hand-packed 64-bit state;
  8 moves + attack + quaff + pickup + open + descend. Invalid actions are
  masked (never sampled) so learning isn't wasted on illegal moves.
  eps 1.0→0.02 (decay 0.995/ep), alpha 0.25, gamma 0.97 (env-overridable).
  Persisted to %LOCALAPPDATA%\DungeonMind\qtable.bin every 10 episodes + at exit.
- Rewards: new tile +0.15 (diminishing), pickup +2, key +12, door opened +6,
  quest +40, kill +(2..14 by tier), descend +40, hit taken -2, wasted-turn -0.02,
  death -60, win +250.
- Episode restart: same-seed by default (clean learning curve), F2 reseeds;
  GUI toggle S switches same/new seed episodes.
- Art: all procedural (pixel images drawn into textures at startup). No downloads.

- Renderer: world draws use MANUAL SCREEN-SPACE PROJECTION, not macroquad
  Camera2D. On software GL (Mesa llvmpipe on GPU-less VMs) miniquad's custom
  camera path submitted geometry that never appeared (verified: 10k+ draws
  per frame with correct coords, black output). Direct projection renders
  identically on hardware and software GL. Do not reintroduce set_camera().
- GPU-less Windows VMs: the VirtIO "Red Hat GPU DOD" has no OpenGL ICD, so
  WGL init fails and the window never opens. Fix: ship Mesa llvmpipe
  opengl32.dll (mmozeiko/build-mesa 26.2.5) next to DungeonMind.exe; the
  normal Windows loader picks it up from the app directory. Verified on
  aarch64; x64 build ships the same way (its GUI can't be demonstrated on
  this ARM VM: the x64 emulator lacks AVX that llvmpipe x64 executes ->
  0xc000001d; fine on real x86_64 or any host with hardware GL).

- Watch mode (feature round 2): training runs in a background thread owning
  its own QAgent; every <=1 s or 100 episodes it publishes Arc<QAgent> into an
  RwLock slot (write lock held only for the pointer swap). The showcase takes
  an Arc clone and only reads, so neither side ever blocks the other.
  Cost: one HashMap clone per publish (~tens of MB at 800k states).
- Speed ladder in src/watch.rs: 0.25x=2 s/turn, 0.5x=1 s, 1x=1/3 s, then linear
  in the multiplier. Motion is tweened in the GUI only (sim stays turn-based).
- macroquad font atlas is 512x512 and doubles when full; the doubling path
  panicked / corrupted glyphs on Mesa llvmpipe. Fix: dynamic text sizes are
  quantized to multiples of 4 and the atlas is pre-warmed for two frames.
  Do not draw text at arbitrary fractional sizes.
- Log lines and boss bar avoid the em dash and unicode ellipsis (not in the
  default font atlas -> box glyph).
