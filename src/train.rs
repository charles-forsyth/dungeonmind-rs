use crate::qlearn::QAgent;
use crate::rng::Rng;
use crate::sim::World;
use std::path::PathBuf;

#[derive(Clone)]
pub struct Config {
    pub seed_mode_fixed: bool,
    pub seed: u64,
    pub max_steps: u32,
}

#[derive(Clone, Debug)]
pub struct EpStats {
    pub reward: f32,
    pub turns: u32,
    pub floor: usize,
    pub won: bool,
    pub dead: bool,
    pub q_size: usize,
    pub eps: f32,
}

pub fn qtable_path() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local).join("DungeonMind").join("qtable.bin")
}

/// Play one full episode; returns stats.
pub fn run_episode(agent: &mut QAgent, cfg: &Config, ep_rng: &mut Rng) -> EpStats {
    let seed = if cfg.seed_mode_fixed {
        cfg.seed
    } else {
        ep_rng.next_u64()
    };
    let mut world = World::new(seed);
    let mut rng = Rng::new(ep_rng.next_u64());
    let mut total = 0f32;
    let mut steps = 0u32;
    loop {
        let obs = world.build_obs();
        let s = obs.key();
        let legal = world.legal();
        let a = agent.choose(s, &legal, &mut rng);
        let (mut r, done) = world.step(a);
        if world.dead {
            r += crate::sim::R_DEATH;
        }
        let obs2 = world.build_obs();
        agent.update(s, a, r, obs2.key(), done);
        total += r;
        steps += 1;
        if trace_on() {
            eprintln!(
                "t{} a{} r{:+.2} f{} hp{}/{} dead{} won{}",
                steps, a, r, world.floor_no, world.hero.hp, world.hero.max_hp, world.dead, world.won
            );
        }
        if done || steps >= cfg.max_steps {
            break;
        }
    }
    EpStats {
        reward: total,
        turns: steps,
        floor: world.deepest_floor(),
        won: world.won,
        dead: world.dead,
        q_size: agent.q.len(),
        eps: agent.eps,
    }
}

pub fn train(n: u32, agent: &mut QAgent, cfg: &Config, mut on_ep: impl FnMut(&EpStats, &EpStats)) {
    // on_ep(latest, ma100)
    let mut rng = Rng::new(0xBADD0DE);
    let mut rewards = std::collections::VecDeque::new();
    let mut floors = std::collections::VecDeque::new();
    let mut wins = std::collections::VecDeque::new();
    for i in 0..n {
        let st = run_episode(agent, &cfg, &mut rng);
        rewards.push_back(st.reward);
        floors.push_back(st.floor);
        wins.push_back(st.won);
        while rewards.len() > 100 {
            rewards.pop_front();
            floors.pop_front();
            wins.pop_front();
        }
        let ma = rewards.iter().sum::<f32>() / rewards.len() as f32;
        let mfa = floors.iter().sum::<usize>() as f32 / floors.len() as f32;
        let wr = wins.iter().filter(|&&w| w).count() as f32 / wins.len() as f32;
        agent.end_episode();
        let ma_st = EpStats {
            reward: ma,
            floor: mfa as usize,
            won: wr > 0.5,
            turns: (wr * 100.0) as u32, // abuse: winrate*100 in turns field for the callback
            dead: false,
            q_size: st.q_size,
            eps: st.eps,
        };
        on_ep(&st, &ma_st);
    }
}

fn trace_on() -> bool {
    static T: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *T.get_or_init(|| std::env::var("DM_TRACE").is_ok())
}
