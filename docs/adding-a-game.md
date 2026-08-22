# Adding a game

This guide walks through adding a second game to the framework. It assumes tic-tac-toe
(`src/games/tictactoe/**`) as the reference implementation and points at the exact traits and
files a new game must supply.

## Overview

The framework owns everything generic: `src/core/**` (the `GameDomain`, `GameRules`,
`GamePrimitives`, `FeatureExtractor`, `Canonicalize`, `StateEvaluator` traits and the feature
algebra), `src/discovery/**` (match engine, corpus generation, annotation, analyzers, tournament
play, the evaluation harness, and the strategy archive), `src/io/**` (the versioned record
schema), and `src/cli/*` (one subcommand per pipeline stage). None of these layers name a
concrete game.

A game supplies everything under `src/games/<game>/**`: rules, primitives, the `game-player`
engine adapter, a `StateEvaluator`, and optionally a `Canonicalize` implementation and tier-2
features, all wired together by one `GameBundle` and one benchmark `Roster`.

| Layer | Owner |
| --- | --- |
| `src/core/**`, `src/discovery/**`, `src/io/**`, `src/cli/*` (except `games.rs`) | framework |
| `src/games/<game>/**` | the game |
| `src/cli/games.rs` | the seam between them |

The one-file game-name rule: `src/cli/games.rs` is the *only* non-test `.rs` file outside
`src/games/` allowed to name a concrete game (its `KNOWN_GAMES` list and `dispatch_game!` macro).
Every other file naming a game must live under `src/games/<game>/` or be a test; the dry-run
checklist below gives the exact `awk` command that checks this.

## Step 1: Domain types

Every game starts with a marker type implementing `GameDomain`, which only names the four
associated types the rest of the framework generically operates over:

```rust
pub trait GameDomain: Send + Sync + 'static {
    type State: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    type Action: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    type Player: Copy + Eq + Hash + Debug + Send + Sync + 'static;
    type Outcome: Clone + Eq + Debug + Send + Sync + 'static;
}
```

`Player` additionally requires `Copy` (players are small, freely duplicated values). For
`CorpusGame` (the blanket bound `src/io/schema.rs` uses to persist a game's records), all four
types must also be `Serialize + DeserializeOwned`; add `#[derive(Serialize, Deserialize)]` to
each from the start. `play`'s transcript output additionally requires `Display` on `State` (see
`src/games/tictactoe/board.rs`'s `impl fmt::Display for Board`).

`State` must be cheap to clone: match simulation is embarrassingly parallel over independent
games (rayon, see cross-cutting rule 2 in `CLAUDE.md`), and no game state is shared mutably across
games. Tic-tac-toe's `Board` is a 10-byte `Copy` struct; prefer flat, `Copy`-able representations
over anything heap-allocated when the board size allows it.

## Step 2: Rules

`GameRules<G>` is the contract for legality, transitions and termination:

- `initial_state(&self) -> G::State`
- `player_to_move(&self, state: &G::State) -> G::Player`
- `legal_actions(&self, state: &G::State) -> Vec<G::Action>` — **empty if and only if** `state`
  is terminal; the framework relies on this to detect game-over without a separate check.
- `apply(&self, state: &G::State, action: &G::Action) -> Result<G::State, RulesError>` — returns
  `Err(RulesError::GameOver)` on a terminal state, `Err(RulesError::IllegalAction(_))` for an
  illegal `action`.
- `outcome(&self, state: &G::State) -> Option<G::Outcome>` — `Some` if and only if `state` is
  terminal.
- `is_terminal(&self, state: &G::State) -> bool` — a default method delegating to `outcome`;
  override only if termination is cheaper to test than the full outcome.

Model rules tests on `src/games/tictactoe/rules.rs`: every win line for both players, full-board
draws, out-of-range and occupied-cell rejections, and a `reachable_positions()` breadth-first
helper used by the tactical and property tests in Step 10.

## Step 3: Engine adapter

`game-player` (jambolo/game-player) is the minimax search engine, not the discovery engine (see
cross-cutting rule 6 in `CLAUDE.md`). Bridging it to a game means implementing `game_player::State`
on the state type and `crate::strategy::engine::EngineGame` on the domain marker type. Copy the
shape of `src/games/tictactoe/engine.rs`:

```rust
impl game_player::State for Board {
    type Action = Move;

    /// Must be unique per reachable position.
    fn fingerprint(&self) -> u64 { /* ... */ }

    fn whose_turn(&self) -> game_player::PlayerId {
        self.to_move().into()
    }

    /// Must agree with `GameRules::is_terminal` for every reachable position.
    fn is_terminal(&self) -> bool { /* ... */ }

    /// Must agree with `GameRules::apply` for every legal action from a nonterminal state.
    fn apply(&self, action: &Move) -> Board { /* ... */ }
}

impl EngineGame for MyGame {
    fn player_id(player: Player) -> game_player::PlayerId { /* map to Alice/Bob */ }
    fn player_from_id(id: game_player::PlayerId) -> Player { /* inverse */ }
    fn winner(outcome: &Outcome) -> Option<Player> { /* None means a draw */ }
}
```

`fingerprint` needs to be unique across every reachable position (tic-tac-toe packs the board into
2 bits per cell plus a side-to-move bit); `whose_turn`, `is_terminal` and `apply` must each agree
exactly with the corresponding `GameRules` method — the tests in Step 10 check this over every
reachable position, not just the fixtures. `EngineGame::winner` returning `None` is how the
framework represents a draw; a game with no draws (see the Konane illustration below) always
returns `Some`.

## Step 4: Evaluator

`StateEvaluator<G>` supplies the non-terminal heuristic used by minimax search and by the
evaluation harness:

```rust
pub trait StateEvaluator<G: GameDomain>: Send + Sync {
    fn evaluate(&self, state: &G::State, perspective: G::Player) -> f32;
}
```

Higher is always better for `perspective`; the framework's `AliceEvaluator` (in
`src/strategy/engine.rs`) wraps a `StateEvaluator` to supply terminal values (`±WIN_VALUE` / `0.0`)
and clamps the heuristic's output so it can never be mistaken for a terminal result. A constant
evaluator is a perfectly valid baseline: `crate::strategy::engine::ConstantEvaluator(0.0)` already
exists and needs no game-specific code — register it under a name like `"zero"` in the bundle's
`evaluators` map (Step 8) alongside a real heuristic. Under `ConstantEvaluator`, every non-terminal
leaf looks equal to depth-limited search, so seeded-uniform tie-breaking spreads play across many
lines — a deliberate corpus-diversity lever (cross-cutting rule 3).

## Step 5: Primitives

`GamePrimitives<G>` declares structural facts about the board with **no strategic insight** —
this is the seam the tier-1 feature vocabulary is derived from mechanically:

- `position_count(&self) -> usize` — addressable positions are `0..position_count()`.
- `adjacent(&self, position: usize) -> Vec<usize>` — neighbors in the board topology, ascending,
  no duplicates.
- `lines(&self) -> Vec<Vec<usize>>` — win-condition lines as position lists, in a stable order
  that becomes the line index used by derived features; no line-based win condition means
  `lines()` returns an empty `Vec`.
- `symmetry_group(&self) -> SymmetryGroup` — the group acting on positions; degree must equal
  `position_count()`.
- `occupant(&self, state: &G::State, position: usize) -> Option<G::Player>`.
- `action_position(&self, action: &G::Action) -> Option<usize>` — the position an action targets,
  if actions address positions at all.
- `transform(&self, state: &G::State, perm: &Permutation) -> G::State` — image of `state` under
  `perm`: the occupant of position `p` moves to `perm.apply(p)`, everything else unchanged.

Nothing here hand-lists "center", "corner" or "edge" — those are orbits of `symmetry_group()`,
derived mechanically by `SymmetryGroup::orbits()` (see
`symmetry_group_orbits_are_centre_corners_edges` in `src/games/tictactoe/primitives.rs`, which
asserts the D4 orbits `[[0,2,6,8], [1,3,5,7], [4]]` without ever naming a corner). Tier-1 features
(per-orbit occupancy counts, per-line mark counts, adjacency-derived counts) fall out of these
primitives for free through `crate::core::derived::PrimitiveFeatures` — a new game gets the full
tier-1 vocabulary just by implementing this trait honestly.

## Step 6: Canonicalization (optional)

`Canonicalize<G>` maps a state to the canonical representative of its symmetry orbit:

```rust
pub trait Canonicalize<G: GameDomain>: Send + Sync {
    fn canonicalize_with_transform(&self, state: &G::State) -> (G::State, Permutation);
    fn canonicalize(&self, state: &G::State) -> G::State { /* default: discard the transform */ }
}
```

It must be **idempotent** (canonicalizing an already-canonical state returns it with the identity
permutation) and **outcome-preserving** (`rules.outcome(canonical) == rules.outcome(state)`, and
player-to-move is unchanged). Tic-tac-toe's canonicalizer takes the minimum base-3 encoding over
all 8 elements of its D4 `symmetry_group()`; the same approach — minimize some total order over
`primitives.transform(state, perm)` for every `perm` in the group — generalizes to any game with a
primitives implementation. Canonicalization buys corpus deduplication and a coverage metric: the
765 canonical tic-tac-toe positions out of 5478 reachable ones feed `known_canonical_positions` on
the bundle (Step 8), which downstream reporting uses as a coverage denominator. A game may skip
`Canonicalize` entirely (`canonicalizer: None` in the bundle) or implement only a partial one.

## Step 7: Tier-2 features (optional)

`FeatureExtractor<G>` is how a game supplies hand-crafted strategic features as a bootstrap for
the miners, never as a ceiling on what they can discover (cross-cutting rule 4):

```rust
pub trait FeatureExtractor<G: GameDomain>: Send + Sync {
    fn definitions(&self) -> Vec<FeatureDef>;
    fn extract(&self, state: &G::State) -> FeatureVector;
}
```

`src/games/tictactoe/features.rs` is the reference: per-line mark counts, `threats.mine` /
`threats.theirs` (lines with two of one side's marks and an empty third cell), `winning_cells`,
`blocking_cells`, `fork_cells` and `win_available`, all prefixed `ttt.` to avoid colliding with
tier-1 primitive feature names. Every definition must be serializable — heuristics emitted later
must carry the definitions of every non-primitive feature they reference (cross-cutting rule 5),
so a `FeatureDef` cannot close over anything that does not round-trip through serde. `extract`
must never panic, including on terminal states.

## Step 8: Bundle and roster

`game_bundle()` is the *only* way a `GameBundle<G>` gets constructed; it is a plain function, not
a trait, and every pipeline stage consumes the resulting `GameBundle` rather than any of the
game's types directly:

```rust
pub fn game_bundle() -> GameBundle<MyGame> {
    let mut evaluators: BTreeMap<String, _> = BTreeMap::new();
    evaluators.insert("default".to_string(), Arc::new(MyGameEvaluator) as Arc<_>);
    evaluators.insert("zero".to_string(), Arc::new(ConstantEvaluator(0.0)) as Arc<_>);

    let roster = benchmark_roster();

    GameBundle {
        name: "my-game".to_string(),
        rules: Arc::new(MyGameRules),
        players: vec![Player::First, Player::Second],
        evaluators,
        default_evaluator: "default".to_string(),
        canonicalizer: Some(Arc::new(MyGameCanonicalizer::new())),
        primitives: Some(Arc::new(MyGamePrimitives)),
        default_strategies: roster.entries.clone(),
        roster,
        full_search_depth: 9,
        known_canonical_positions: Some(765),
    }
}
```

Every field matters: `name` (must equal the config's `game` field), `rules`, `players` (turn
order — `players[0]` moves first), `evaluators` plus `default_evaluator` (the key used when no
evaluator is named explicitly), `canonicalizer` and `primitives` (both `Option`, `None` when a
game skips Steps 5/6), `default_strategies` (conventionally `roster.entries.clone()`), `roster`
itself, `full_search_depth` (an engine depth that is exhaustive for this game — 9 plies for
tic-tac-toe), and `known_canonical_positions` (the coverage denominator from Step 6, `None` when
there is no canonicalizer or the count is unknown).

`benchmark_roster()` builds the versioned opponent population the Phase 7 evaluation harness plays
against, via `Roster::graded(name, version, &depths, perfect_depth)`:

```rust
pub fn benchmark_roster() -> Roster {
    Roster::graded("my-game-benchmark", 1, &[1, 2, 3, 4, 6], 9)
        .expect("constant benchmark roster is valid")
}
```

`graded` builds a `random` baseline, one seeded-uniform `depth-{d}` minimax entry per depth in
`&depths`, then a `perfect` entry at `perfect_depth`. Bump the roster version whenever entries
change, so archived evaluation results stay comparable only within the version that produced them.
`roster.id()` (`"{name}-v{version}"`) is what `evaluation.json` records as provenance.

## Step 9: Register the game

The only change outside `src/games/` is in `src/cli/games.rs`: one string in `KNOWN_GAMES` and one
arm in the `dispatch_game!` macro.

```rust
pub const KNOWN_GAMES: &[&str] = &["tictactoe", "my-game"];

macro_rules! dispatch_game {
    ($name:expr, |$bundle:ident| $body:expr) => {
        match $name {
            "tictactoe" => {
                let $bundle = &$crate::games::tictactoe::game_bundle();
                $body
            }
            "my-game" => {
                let $bundle = &$crate::games::my_game::game_bundle();
                $body
            }
            other => Err($crate::cli::games::unknown_game(other).into()),
        }
    };
}
```

Nothing else in `src/cli/` changes: `play`, `generate`, `annotate`, `analyze`, `report`,
`pipeline`, and `evaluate` all resolve their `--game` argument (or a config's `game` field, or a
run directory's `run.json`) through `dispatch_game!`, so every subcommand works for the new game
immediately once this one arm exists.

## Step 10: Tests

Bring at least these, mirroring the tic-tac-toe suite:

- **Rules fixtures** (`src/games/<game>/rules.rs`, `#[cfg(test)]`): every win line for every
  player, draws (if the game has them), illegal-action rejections (occupied/out-of-range targets,
  moves after game-over).
- **Tactical regressions** (`tests/`, mirroring `tests/minimax_tactics.rs`): immediate-win and
  immediate-block fixtures at shallow depth, and a perfect-vs-perfect check that the outcome is
  stable across multiple tie-break seeds (a draw for tic-tac-toe; whatever the solved result is
  for the new game).
- **The generic property tests re-instantiated** for the new game (`tests/invariants.rs` is the
  tic-tac-toe copy to clone and retarget): `terminal_states_have_no_legal_moves`,
  `legal_moves_preserve_board_validity`, `canonicalization_is_idempotent` (only if Step 6 was
  done), `canonicalization_preserves_outcome` (same caveat).
- **Non-failing micro-benchmarks**, logged rather than asserted, from Phase 2's benchmarking
  convention onward.

## Worked illustration: Konane

This section is illustrative only — no Konane implementation ships in this milestone — but it
exercises every "optional" and "it depends" corner of Steps 1–8 on a genuinely different game.

Konane (Hawaiian checkers) is played on a rectangular board, filled alternately black and white
with no empty squares; here take a 6x6 board, `position_count() == 36`. Play opens by each side
removing one of their own stones from the board (specific opening squares are traditional but not
required for the illustration), then proceeds as orthogonal jump captures: a stone jumps an
adjacent enemy stone into the empty square immediately beyond it, capturing the jumped stone;
multi-jumps continue in a straight line as long as each individual jump is legal. The player who
*cannot* move loses — there is no line-based win condition and no draws, so:

- `Outcome` is `Win(Player)` only; `winner` (Step 3) is always `Some`, never `None`.
- `lines()` (Step 5) returns an empty `Vec` — Konane has no win-lines to derive tier-1 line
  features from; only orbit- and adjacency-derived tier-1 features apply.
- `adjacent` (Step 5) is the four orthogonal neighbors (no diagonals — jumps are orthogonal only).
- `action_position` (Step 5) is the *landing* square of a (possibly multi-) jump, not the origin;
  a game with multi-jumps needs to decide this convention up front and hold to it consistently.
- The symmetry group (Step 5) is D4 only because the board is 6x6 (square); a non-square board
  would be limited to the smaller group of rotations/reflections that actually preserve its shape.
  Critically, the group must be chosen on the *board*, not the *coloring*: a 90-degree rotation of
  an alternating black/white fill swaps which squares are which color, so a naive D4 canonicalizer
  built on the colored board is unsound. Canonicalization for Konane is therefore optional/partial
  at best — either restrict to the symmetries that also preserve the coloring, or skip Step 6
  (`canonicalizer: None`) and accept no deduplication.
- `Roster::graded` (Step 8) should use depths suited to Konane's branching factor, which is higher
  than tic-tac-toe's once multi-jumps are counted as single moves — shallower depths per entry
  than tic-tac-toe's `&[1, 2, 3, 4, 6]` are likely appropriate.
- `full_search_depth` (Step 8) is *not* exhaustive for 6x6 Konane the way depth 9 is exhaustive for
  tic-tac-toe's 9 cells — the game tree is far larger. Consequently `annotate --exhaustive` is not
  a viable option for this game; `evaluate --annotations` must instead point at corpus-derived
  annotations (`annotate --corpus`) rather than an exhaustive set.

## Second-game dry-run checklist

Run this checklist against the actual second game as it is implemented. Each item names the
files it constrains and the command that verifies it.

- [ ] Every changed non-test source file is under `src/games/<game>/**`, except exactly one
  `KNOWN_GAMES` entry and one `dispatch_game!` arm in `src/cli/games.rs`. Verify with:

```sh
git diff --stat -- src/discovery src/io src/core src/cli
```

  This must show only `src/cli/games.rs` (or no output at all, if Step 9 is not yet staged).

- [ ] No file outside `src/games/<game>/` and outside `src/cli/games.rs` names the new game, in
  non-test code. Verify with (replace `<game>` with the literal game name both places it appears):

```sh
for f in $(grep -rl <game> src --include=*.rs | grep -v '^src/games/' | grep -v '^src/cli/games.rs$'); do awk '/#\[cfg\(test\)\]/{t=1} /<game>/ && !t {print FILENAME": "NR; v=1} END{exit v}' $f || echo "FAIL $f"; done
```

  No output means every remaining mention is inside a `#[cfg(test)]` module.

- [ ] New test files exist under `tests/` for the rules, tactical regressions, and re-instantiated
  property tests from Step 10; nothing under `tests/` for the old game changed.

- [ ] `docs/adding-a-game.md` (this file) and any game-specific ADR the second game motivates are
  the only documentation changes.

- [ ] The generic property invariants hold for the new game (retarget `tests/invariants.rs` or add
  a sibling file):

```sh
cargo test --test invariants
```

- [ ] The new game plays end to end through the CLI with no pipeline change:

```sh
cargo run -- play --game <game> --players random,perfect --games 2
```

- [ ] The corpus pipeline runs end to end for the new game:

```sh
cargo run -- generate --config <config>.toml --out <dir>
cargo run -- annotate --corpus <dir>
cargo run -- analyze --corpus <dir>
```

- [ ] The evaluation harness runs for the new game and produces a well-formed report without any
  pipeline-level code change:

```sh
cargo run -- evaluate --game <game> --strategies <file> --out <dir>
```

  `<dir>/evaluation.json` must exist, and its `roster.name` / `roster.version` must be the new
  game's `benchmark_roster()` identity, not tic-tac-toe's — confirming the evaluate stage picked
  up the new bundle purely through `dispatch_game!` (Step 9), with no `src/discovery/**` change.

- [ ] All five quality gates pass:

```sh
cargo check --workspace --all-targets
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --no-deps --workspace
```
