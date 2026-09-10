# ADR 0011: Strategy evaluation harness

## Status

Accepted — 2026-08-22 (Milestone 1 / plan Phase 7)

## Context

- `docs/plan.md` Phase 7 (Gate C) requires evaluating any registered strategy against the game's
  full graded benchmark population and reporting headline metrics — loss rate against a reference
  opponent, win/draw/loss rates against every weaker opponent, and move-agreement rate against
  annotated best moves — persisted with provenance, before the archive (ADR 0012) or any mining
  work can build on it.

- Before this milestone the crate had `play` (one pairing, one seed, no tallying against a
  population), the corpus-side `agreement` analyzer (which measures what was actually played in a
  fixed corpus, joined by raw `state`), and graded rosters (`Roster::graded`, tic-tac-toe's
  `ttt-benchmark-v1` = `random`, `depth-1`, `depth-2`, `depth-3`, `depth-4`, `depth-6`, `perfect`).
  Nothing scored an arbitrary strategy against that population.

- A strategy-side harness is distinct from the corpus-side `agreement` analyzer: the analyzer
  reports what strategies already produced a corpus; this harness plays a strategy under test
  itself, fresh, against a roster and, optionally, against every non-terminal position of an
  annotation set, so a strategy that never contributed to any corpus can still be scored.

- Tic-tac-toe's own caveat motivates the agreement metric: against `perfect` every sound strategy
  draws every game, so win/loss rate alone cannot separate two strategies that both never lose;
  agreement with the annotated optimal action, and the graded weaker-opponent rates, are what
  differentiate them.

- ADR 0002 fixed `splitmix64`/`game_seed`/`player_seed` seed derivation and the serial/threads/pool
  determinism invariant; ADR 0009/0010 fixed the record schema, byte-identity and the CLI/exit-code
  contract this harness extends rather than replaces.

## Decision

- Modules, all game-neutral (generic over `G: EngineGame + CorpusGame`, taking `&GameBundle<G>`,
  naming no concrete game): `src/discovery/tournament.rs` (roster play and tallies),
  `src/discovery/evaluate.rs` (annotation loading, strategy-side agreement, behavior signature,
  report assembly, strict check), `src/discovery/archive.rs` (ADR 0012), and CLI `src/cli/
  evaluate.rs`; game resolution happens only through `dispatch_game!`. `GameBundle` gains a
  `roster: Roster` field — the versioned default benchmark population — whose `roster.id()`
  (`"{name}-v{version}"`) is the provenance `roster_id` recorded on every tournament. Non-generic
  record types live in `src/io/schema.rs`; new file constants `EVALUATION_FILE = "evaluation.json"`,
  `ARCHIVE_FILE = "archive.json"`, `ARCHIVE_ENTRIES_FILE = "entries.jsonl"`; every document carries
  `schema_version = 1`. No existing record, constant, seed formula or CLI line changes.

- Pairings: for every roster entry in roster order (`opponent_index`) and every seat
  `seat in 0..bundle.players.len()` (tic-tac-toe: seat 0 = X, seat 1 = O; an N-player game gets N
  seatings), the strategy under test occupies `bundle.players[seat]` and the opponent occupies
  every other seat; `pairing_index = opponent_index * seats + seat`. One `MatchEngine::run` per
  pairing plays `games_per_pairing` games through `RayonMatchEngine::{serial, with_threads, new}`.
  The under-test entry may coincide with a roster entry — it then plays a fresh clone of itself.

- Seeds: each pairing's batch seed is `cell_seed(seed, opponent_index * seats + seat)`, the
  existing `splitmix64(master ^ 0x5851F42D4C957F2D ^ (index as u64).wrapping_mul(0xD1B54A32D192ED03))`
  cell-seed formula; within a batch the existing `game_seed`/`player_seed` derivation of ADR 0002
  applies unchanged, so results never depend on `--serial` versus `--threads N` versus the global
  pool. The agreement pass (below) derives its own per-position seed,
  `game_seed(splitmix64(seed ^ 0x9E3779B97F4A7C15), index)`, from the same master seed via a
  distinct salt so the two passes never collide.

- Classification: per game, from `MatchRecord.outcome` via `G::winner`: winner
  `== bundle.players[seat]` is a win, any other winner is a loss, no winner is a draw, and
  `outcome: None` (the game was aborted by `max_plies`) is unfinished — never a loss. Every rate is
  `count / games`, exactly `0.0` when `games == 0`. Every tally is built through
  `Tally::from_counts`. Results are `Vec`s in roster/seat order, never maps, so document order is
  reproducible without a secondary sort key.

- Errors never become losses: any `MatchError` aborts the whole tournament as
  `CorpusError::Match`, so a strategy kind that cannot yet play — `heuristic-rules`, whose
  interpreter is Milestone 2 — surfaces as `StrategyError::Unimplemented` (exit 2), not as a
  tallied loss. Every provider, the strategy under test and every roster entry, is built once up
  front before any game is played, so a build failure for any of them also surfaces before partial
  results exist.

- Reference opponent: `--reference NAME`, defaulting to the last roster entry (graded rosters end
  in `perfect`). The name must match a roster entry or the run fails with `CorpusError::Config`
  (exit 2). `headline.loss_rate_vs_reference` is the loss rate of the `OpponentResult` whose
  `opponent == reference`.

- Agreement (strategy-side, distinct from the corpus-side `agreement` analyzer): `--annotations
  DIR` names a directory holding `annotations.jsonl` and `annotate.json`, the output of either
  `annotate --corpus DIR` or `annotate --exhaustive --out DIR`; a missing file is
  `CorpusError::Precondition`, naming the missing file and both commands that produce it. For every
  record with `terminal == false`, in file order, with `index` its 0-based position in the file
  (terminal records still occupy an index but are skipped): `legal = rules.legal_actions(state)`, a
  brand-new strategy instance is built via `provider.create(game_seed(splitmix64(seed ^
  AGREEMENT_SEED_SALT), index))`, `chosen = strategy.choose(state, legal)`, and the position agrees
  iff `optimal_actions.contains(&chosen)`. Positions are evaluated over rayon and collected back
  into index order; any `StrategyError` aborts the pass. The result is `AgreementSummary {
  positions, agreeing, rate, by_value: BTreeMap<i8, AgreementCounts> }`, keyed by the record's
  side-to-move value (-1, 0, 1). Depth-9 minimax agrees at rate 1.000 on all 4520 non-terminal
  exhaustive positions and on all 390 `generate-small` corpus annotations; `random` does not.

- Behavior signature: sampled in the same annotation pass, as the input to ADR 0012's behavior
  distance. Let `n` be the non-terminal record count and `SIGNATURE_SAMPLE = 256`; the stride is
  `max(1, n / 256)`, and the sampled ordinals are `0, stride, 2*stride, ...` among the non-terminal
  records, taken while fewer than 256 have been collected and the ordinal is still `< n`. The
  result is `BehaviorSignature { sample_id, actions: Vec<serde_json::Value> }` where `sample_id` is
  the `config_hash` of `{ mode, run_id, nonterminal, stride }`, so signatures from differently
  shaped annotation sets are never mistaken for comparable. The signature is `None` whenever no
  `--annotations` were supplied.

- Evaluation document `evaluation.json` (pretty JSON plus a trailing newline), one entry per
  strategy in strategies-file order, with `evaluation_id` the `config_hash` (FNV-1a 64, hex16) of
  `{ game, roster, config, strategies }`:

  ```text
  {
    schema_version, evaluation_id, game, roster,
    config: { evaluator, games_per_pairing, seed, max_plies, reference,
              annotations: { mode, run_id, records } | null },
    strategies: [
      {
        name, kind, spec, spec_hash,
        tournament: { roster_id, reference, games_per_pairing, seed,
                      opponents: [ { opponent, games, wins, draws, losses, unfinished,
                                     win_rate, draw_rate, loss_rate,
                                     by_seat: [ { opponent, seat, player, games, ... } ] } ],
                      totals },
        headline: { reference, loss_rate_vs_reference, win_rate, draw_rate, loss_rate,
                    games, unfinished, agreement_rate },
        agreement | null,
        signature | null
      }
    ]
  }
  ```

  No paths, timestamps, durations, thread counts or hostnames appear in any output document; every
  map is a `BTreeMap` so key order is stable.

- CLI: `evaluate --game NAME --strategies FILE --out DIR [--archive DIR (default <out>/archive)]
  [--annotations DIR] [--roster FILE] [--reference NAME] [--games N (20)] [--seed S (0)]
  [--max-plies M] [--evaluator NAME] [--strict] [--max-loss-rate F (0.0)] [--serial]
  [--threads N]`, plus the global `-v`/`-q`. Flow: every strategy is fully evaluated before
  anything is written, so a runtime error writes no file; then `evaluation.json` is written, then
  the archive gets one appended entry per strategy, then stdout, then (last) the `--strict` check.

- stdout is frozen to two line shapes, one per strategy followed by one summary line:

  ```text
  strategy={name} kind={kind} games={n} wins={w} draws={d} losses={l} unfinished={u} loss_rate_vs_reference={:.3} agreement={:.3 or none} novelty={:.3}
  evaluation={evaluation_id} game={game} roster={roster_id} reference={name} strategies={n} archive_entries={n} out={dir}
  ```

  `archive_entries` is the archive's total count after this run completes, so a repeat run into the
  same archive prints the same bytes (ADR 0012 makes the append a no-op on a duplicate entry);
  `novelty` is the archived entry's `novelty.distance`. Only the `out=` field's value tracks the
  `--out` flag; verbosity (`-v`/`-q`) never changes stdout, only stderr logging.

- Exit codes, extending the ADR 0010 taxonomy with no renumbering:

| exit | class | when |
| --- | --- | --- |
| 0 | success | the run completed; with `--strict`, every strategy was within `--max-loss-rate` |
| 1 | failure | a runtime error during play: engine or I/O failure at match time |
| 2 | usage | missing/malformed input files, unknown game, evaluator, reference or strategy kind, `StrategyError::Unimplemented`, a schema-version mismatch, or an archive whose recorded game does not match `--game` |
| 3 | check | `--strict` and at least one strategy's loss rate against the reference exceeded `--max-loss-rate`; `CliError::CheckFailed` reports every offending strategy as `error: check failed: reference loss rate exceeded: {name} {:.3} > {:.3}` entries joined by `; `, applied only after every file and archive entry has already been written, mirroring `analyze --strict` |

- Byte-identity, extending ADR 0009/0010: `evaluation.json`, `archive.json`, `entries.jsonl` and
  stdout are byte-identical across repeat runs, across `--serial` versus `--threads N` versus the
  global pool, and with or without `-v` (only the `out=` echo tracks the flag). A repeat run into
  an existing archive appends nothing new (ADR 0012 dedupes by content). Evidence lives in
  `tests/cli_evaluate.rs`, the unit tests in `src/discovery/tournament.rs` and
  `src/discovery/evaluate.rs`, `tests/tournament_bench.rs`, and, from Milestone 1 Phase 4,
  `artifacts/reproducibility/tournament-determinism.md` and
  `artifacts/benchmarks/tournament-throughput.md`.

## Consequences

- Any registered strategy kind — including a future `heuristic-rules` once its interpreter lands,
  and any strategy a later mining phase generates — is scored by the same `evaluate` command with
  no harness change; a second game needs only to populate its own bundle's `roster`.

- Exhaustive or corpus annotation is an optional input, computed by an earlier stage and never
  implicitly triggered by `evaluate`, so evaluation stays cheap when agreement is not needed and
  the harness never silently runs a second pipeline stage as a side effect.

- The headline separates two strategies that both draw every game against `perfect` (the
  tic-tac-toe case) by their rates against weaker roster entries and by agreement rate, so "sound"
  and "distinguishable" are both answerable from one document.

- Full-roster runs stay cheap: the roster's `depth-9`/`perfect` opponents make a full-population
  tournament run in seconds even in a debug build, while the agreement pass over up to 4520
  exhaustive positions scales with the strategy under test's own per-move cost, not with the
  roster.

- Treating any `MatchError` (including `StrategyError::Unimplemented`) as a hard error rather than
  a loss keeps "this strategy cannot play yet" and "this strategy plays badly" from being
  conflated in the same headline number.

- Evidence: `tests/cli_evaluate.rs`, `src/discovery/tournament.rs`, `src/discovery/evaluate.rs`,
  `tests/tournament_bench.rs`.
