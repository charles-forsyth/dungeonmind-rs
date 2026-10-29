# Contributing to DungeonMind (Rust)

We welcome contributions to the reinforcement learning agent, simulation engine, or rendering frontend!

## Getting Started

1. **Prerequisites**: Ensure you have a recent Rust toolchain installed (`rustup update stable`).
2. **Branching**: Branch off `main` for all features or fixes.
3. **Testing**: Run unit and simulation tests:
   ```bash
   cargo test --test sim
   ```
4. **Code Quality**:
   - Run `cargo fmt --check` and `cargo clippy`.
   - Ensure the reinforcement learning invariants in `tests/sim.rs` pass.
5. **Pull Requests**: Open a pull request against `main`. All PRs are squash-merged after review.
