mod gui;
mod gui_art;
use dungeonmind::qlearn::QAgent;
use dungeonmind::rng::Rng;
use dungeonmind::train::{self, Config, EpStats};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let headless = args.iter().any(|a| a == "--headless");
    let mut train_n: Option<u32> = None;
    let mut fixed_seed = false;
    let mut seed: u64 = 1337;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--train" {
            train_n = it.next().and_then(|s| s.parse().ok());
        } else if a == "--seed" {
            seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(seed);
        } else if a == "--fixed-seed" {
            fixed_seed = true;
        }
    }
    let path = train::qtable_path();
    if let Some(n) = train_n {
        let mut agent = QAgent::new();
        if agent.load(&path) {
            println!(
                "loaded q-table: {} states, {} episodes, eps {:.3}",
                agent.q.len(),
                agent.episodes,
                agent.eps
            );
        }
        let cfg = Config {
            seed_mode_fixed: fixed_seed,
            seed,
            max_steps: std::env::var("DM_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(2000),
        };
        let t0 = std::time::Instant::now();
        let log_every = (n / 40).max(1);
        let mut best_floor = 0usize;
        let mut wins = 0u32;
        let mut ep = 0u32;
        let mut last_ma = 0f32;
        train::train(n, &mut agent, &cfg, &mut |st: &EpStats, ma: &EpStats| {
            ep += 1;
            last_ma = ma.reward;
            best_floor = best_floor.max(st.floor);
            if st.won {
                wins += 1;
            }
            if ep % log_every == 0 || st.won {
                println!(
                    "ep {:>5}  r {:>9.1}  ma100 {:>8.1}  floor {}  eps {:.3}  q {:>7}  turns {}  {}",
                    ep,
                    st.reward,
                    ma.reward,
                    st.floor,
                    st.eps,
                    st.q_size,
                    st.turns,
                    if st.won { "WIN" } else { "" }
                );
            }
        });
        agent.save(&path).ok();
        println!(
            "done: {} episodes in {:.1}s, wins {}, best floor {}, final ma100 {:.1}, table {} states -> {}",
            n,
            t0.elapsed().as_secs_f64(),
            wins,
            best_floor,
            last_ma,
            agent.q.len(),
            path.display()
        );
        return;
    }
    if headless {
        let mut agent = QAgent::new();
        agent.load(&path);
        let mut rng = Rng::new(1);
        let st = train::run_episode(
            &mut agent,
            &Config {
                seed_mode_fixed: fixed_seed,
                seed,
                max_steps: 2000,
            },
            &mut rng,
        );
        println!(
            "episode: reward {:.1} turns {} floor {} dead {} won {} q {}",
            st.reward, st.turns, st.floor, st.dead, st.won, st.q_size
        );
        return;
    }
    gui::main();
}
