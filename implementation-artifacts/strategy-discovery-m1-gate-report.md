# strategy-discovery-m1 — Project gate report

- date: 2026-08-23
- commit: 216583a62e7aaf550c16f6e11b25d5640847be9d
- branch: milestone/1-evaluation-harness

## Project Definition of Done

### Item 1 — Quality gates and budget

```text
command: cargo check --workspace --all-targets; echo "exit=$?"
expected: exit=0
actual: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s
        exit=0

command: cargo test --workspace > ws-test.txt 2>&1; echo "exit=$?"; grep -c "test result: FAILED" ws-test.txt; grep -c "test result: ok" ws-test.txt
expected: exit=0, FAILED count=0, ok count=21
actual: exit=0
        FAILED count=0
        ok count=21

command: cargo fmt --all --check; echo "exit=$?"
expected: exit=0
actual: exit=0

command: cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1 | tail -2; echo "exit=${PIPESTATUS[0]}"
expected: exit=0
actual: Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.21s
        exit=0

command: cargo doc --no-deps --workspace 2>&1 | grep -c "^warning"
expected: 0
actual: 0

command: pwsh -NoProfile -Command "(Measure-Command { cargo test --workspace 2>&1 | Out-Null }).TotalSeconds" (everything already built)
expected: <= 60 (budget)
actual: 41.4482811
verdict: PASS
```

### Item 2 — Property-based invariants

```text
command: grep -rl "proptest!" tests/ src/ | wc -l
expected: >= 1
actual: 1 (tests/invariants.rs)

command: cargo test --test invariants 2>&1 | grep -E "^test (terminal_states_have_no_legal_moves|legal_moves_preserve_board_validity|canonicalization_is_idempotent|canonicalization_preserves_outcome|feature_expr_serde_round_trips|heuristic_strategy_serde_round_trips|strategy_spec_serde_round_trips) \.\.\. ok$" | wc -l
expected: 7, and test result: ok. 7 passed; 0 failed
actual: 7
        test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
verdict: PASS
```

### Item 3 — Full-scale run

```text
command: cargo run -q -- annotate --exhaustive --game tictactoe --out target/m1/s24/exhaustive
expected: mode=exhaustive annotated=5478 terminal=958 disagreements=0
actual: mode=exhaustive annotated=5478 terminal=958 disagreements=0

command: cargo run -q -- evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m1/s24/exhaustive --out target/m1/s24/eval > stdout-1.txt; echo "exit=$?"
expected: exit=0
actual: exit=0
        strategy=perfect kind=minimax games=280 wins=87 draws=193 losses=0 unfinished=0 loss_rate_vs_reference=0.000 agreement=1.000 novelty=1.000
        strategy=random kind=random games=280 wins=25 draws=26 losses=229 unfinished=0 loss_rate_vs_reference=0.875 agreement=0.579 novelty=0.535
        strategy=depth-3 kind=minimax games=280 wins=87 draws=146 losses=47 unfinished=0 loss_rate_vs_reference=0.150 agreement=0.978 novelty=0.051
        evaluation=44ad0fffbef146da game=tictactoe roster=ttt-benchmark-v1 reference=perfect strategies=3 archive_entries=3 out=target/m1/s24/eval

command: ls target/m1/s24/eval/evaluation.json target/m1/s24/eval/archive/archive.json target/m1/s24/eval/archive/entries.jsonl | wc -l
expected: 3
actual: 3

command: pwsh -NoProfile -ExecutionPolicy Bypass -File target/m1/s24/check-eval.ps1 -Path target/m1/s24/eval/evaluation.json
expected: roster_id=ttt-benchmark-v1 roster_name=ttt-benchmark strategies=3 opponents=7; pairings_per_strategy=14,14,14 pairings_not_20_games=0; perfect_opponents_with_nonzero_loss_rate=0 perfect_loss_rate_vs_reference=0; random/depth3 loss rates recorded; agreement line used by item 4
actual: roster_id=ttt-benchmark-v1 roster_name=ttt-benchmark strategies=3 opponents=7
        pairings_per_strategy=14,14,14 pairings_not_20_games=0
        perfect_opponents_with_nonzero_loss_rate=0 perfect_loss_rate_vs_reference=0
        random_loss_rate_vs_reference=0.875 depth3_loss_rate_vs_reference=0.15
        perfect_agreement_rate=1 perfect_agreement_positions=4520 random_agreement_rate=0.5789823008849557

command: grep -c "^strategy=" stdout-1.txt; grep -c "^evaluation=" stdout-1.txt
expected: 3, 1
actual: 3, 1
verdict: PASS
```

### Item 4 — Agreement

```text
command: (from item-3 script output)
expected: perfect_agreement_rate=1 perfect_agreement_positions=4520 random_agreement_rate=<1
actual: perfect_agreement_rate=1 perfect_agreement_positions=4520 random_agreement_rate=0.5789823008849557

command: grep -c "^strategy=perfect .* agreement=1.000 " stdout-1.txt
expected: 1
actual: 1
verdict: PASS
```

### Item 5 — Byte identity across serial/pool/thread-count runs

```text
command: four evaluate runs (eval2 fresh dir, eval same dir 3rd run, eval-s --serial, eval-t4 --threads 4), each "; echo exit=$?"
expected: exit=0 for all four
actual: exit=0, exit=0, exit=0, exit=0

command: for f in evaluation.json archive/archive.json archive/entries.jsonl: sha256sum eval/$f eval2/$f eval-s/$f eval-t4/$f | awk '{print $1}' | sort -u | wc -l
expected: 1 for each
actual: evaluation.json -> 1 (hash 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a)
        archive/archive.json -> 1 (hash 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a)
        archive/entries.jsonl -> 1 (hash 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8)

command: wc -l < target/m1/s24/eval/archive/entries.jsonl
expected: 3 (no duplicate after third run)
actual: 3

command: for s in 2 3 s t4; do diff <(sed 's/ out=.*$//' stdout-1.txt) <(sed 's/ out=.*$//' stdout-$s.txt) > /dev/null && echo "stdout-$s same"; done
expected: four "same" lines
actual: stdout-2 same
        stdout-3 same
        stdout-s same
        stdout-t4 same
verdict: PASS
```

### Item 6 — Archive

```text
command: wc -l < entries.jsonl
expected: 3 (>= 2)
actual: 3

command: grep -c '"provenance":{"game":"tictactoe","source":"evaluate","evaluation_id":"[0-9a-f]*","roster_id":"ttt-benchmark-v1","seed":0,"games_per_pairing":20,"evaluator":"default","corpus_run_id":null,"annotations_run_id":null,"annotations_mode":"exhaustive"}' entries.jsonl
expected: 3
actual: 3

command: grep -c '"novelty":{"method":"m1-v1","nearest_entry_id":' entries.jsonl; grep -c '"spec_distance":' entries.jsonl; grep -c '"is_novel":' entries.jsonl
expected: 3, 3, 3
actual: 3, 3, 3

command: cargo test --lib archive 2>&1 | grep "test result"
expected: ok. 7 passed (includes open_append_duplicate_and_reopen_round_trip)
actual: test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 313 filtered out; finished in 0.01s
        test discovery::archive::tests::open_append_duplicate_and_reopen_round_trip ... ok

command: pwsh -NoProfile -Command "Get-Content -Raw archive.json | ConvertFrom-Json | Out-Null; Get-Content entries.jsonl | ForEach-Object { $_ | ConvertFrom-Json | Out-Null }; 'json-ok'"
expected: json-ok
actual: json-ok
verdict: PASS
```

### Item 7 — Error paths

```text
command: cargo run -q -- evaluate --game tictactoe --strategies tests/fixtures/strategies-heuristic.toml --roster tests/fixtures/roster-tiny.toml --games 1 --out target/m1/s24/heur 2> heur-err.txt; echo "exit=$?"
expected: exit=2
actual: exit=2
        error: evaluating strategies from tests/fixtures/strategies-heuristic.toml: strategy `heuristic-rules` is not implemented: rule interpreter execution lands with heuristic mining (plan.md Phase 8)

command: grep -c 'heuristic-rules' heur-err.txt; grep -c 'not implemented' heur-err.txt; test -e target/m1/s24/heur/evaluation.json; echo "exists=$?"
expected: 1, 1, exists=1
actual: 1, 1, exists=1

command: cargo run -q -- evaluate ... --roster tests/fixtures/roster-tiny.toml --games 2 --strict --out target/m1/s24/strict > /dev/null 2> strict-err.txt; echo "exit=$?"
expected: exit=3, stderr "error: check failed: reference loss rate exceeded: random 0.750 > 0.000"
actual: exit=3
        error: check failed: reference loss rate exceeded: random 0.750 > 0.000

command: grep -c '^error: check failed: reference loss rate exceeded: random ' strict-err.txt
expected: 1
actual: 1

command: cargo run -q -- evaluate --game chess --strategies tests/fixtures/strategies-eval.toml --out target/m1/s24/u1 2>/dev/null; echo "exit=$?"
expected: exit=2
actual: exit=2

command: cargo run -q -- evaluate --game tictactoe --strategies tests/fixtures/nope.toml --out target/m1/s24/u2 2>/dev/null; echo "exit=$?"
expected: exit=2
actual: exit=2

command: cargo run -q -- evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --roster tests/fixtures/roster-tiny.toml --games 1 --reference nobody --out target/m1/s24/u3 2>/dev/null; echo "exit=$?"
expected: exit=2
actual: exit=2
verdict: PASS
```

### Item 8 — Tournament throughput benchmark harness

```text
command: test -f tests/tournament_bench.rs && echo ok
expected: ok
actual: ok

command: cargo test --test tournament_bench -- --nocapture 2>&1 | grep "^\[bench\]" | tee bench.txt | wc -l
expected: 6
actual: 6

command: grep -c "games/s=" bench.txt; grep -c "us/move=" bench.txt; grep -c "threads=" bench.txt
expected: 5, 5, 5
actual: 5, 5, 5

command: (same run's) test result
expected: ok. 2 passed
actual: test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.29s
verdict: PASS
```

### Item 9 — Docs

```text
command: grep -c '^## Second-game dry-run checklist$' docs/adding-a-game.md; sed -n '/^## Second-game dry-run checklist$/,$p' docs/adding-a-game.md | grep -c '^- \[ \] '; grep -c '^## Worked illustration: Konane$' docs/adding-a-game.md
expected: 1, >= 8, 1
actual: 1, 9, 1

command: for each ADR 0011/0012/0013: grep -cE '^## (Status|Context|Decision|Consequences)$' F
expected: 4 for each
actual: 0011 -> 4, 0012 -> 4, 0013 -> 4

command: grep -c '^### evaluate$' README.md; grep -c '^cargo run -- <subcommand>   # CLI stages: play, generate, annotate, analyze, report, evaluate, pipeline$' CLAUDE.md
expected: 1, 1
actual: 1, 1

command: for f in $(grep -rl tictactoe src --include=*.rs | grep -v '^src/games/' | grep -v '^src/cli/games.rs$'); do awk '/#\[cfg\(test\)\]/{t=1} /tictactoe/ && !t {print FILENAME": "NR; v=1} END{exit v}' $f || echo "FAIL $f"; done
expected: no output
actual: (no output)
verdict: PASS
```

### Item 10 — Evidence artifacts

```text
command: for F in tournament-determinism.md, tournament-throughput.md: grep -cE '^\| (date|git_commit|rustc|os|cpu|ram|build_profile|command) \| ' F
expected: 8 for each
actual: tournament-determinism.md -> 8, tournament-throughput.md -> 8

command: grep -c 'identical: YES' artifacts/reproducibility/tournament-determinism.md; grep -c 'identical: NO' artifacts/reproducibility/tournament-determinism.md
expected: >= 4, 0
actual: 6, 0

command: pwsh -NoProfile -Command "Get-Content -Raw artifacts/benchmarks/tournament-throughput.json | ConvertFrom-Json | Out-Null; 'json-ok'"
expected: json-ok
actual: json-ok

command: tail -1 docs/component-evaluation.md
expected: Gate C verdict: PASS
actual: Gate C verdict: PASS

command: git diff dcd22b1 --stat -- docs/plan.md docs/adr/0001-game-player-minimax-adapter.md docs/adr/0010-cli-contract-and-analyzer-registry.md artifacts/benchmarks/corpus-generation.md artifacts/reproducibility/cli-outputs-determinism.md configs | wc -l
expected: 0
actual: 0
verdict: PASS
```

### Item 11 — Cargo.toml diff is exactly the harness dependencies

```text
command: git diff dcd22b1 -- Cargo.toml | grep -E '^[-+]' | grep -vE '^(\+\+\+|---)'
expected: -serde_json = "1.0.151"
          +serde_json = { version = "1.0.151", features = ["float_roundtrip"] }
          +
          +[dev-dependencies]
          +assert_cmd = "2.2.2"
          +proptest = "1.11.0"
          +tempfile = "3.27.0"
actual: -serde_json = "1.0.151"
        +serde_json = { version = "1.0.151", features = ["float_roundtrip"] }
        +
        +[dev-dependencies]
        +assert_cmd = "2.2.2"
        +proptest = "1.11.0"
        +tempfile = "3.27.0"

command: grep -c float_roundtrip Cargo.toml; git diff dcd22b1 -- Cargo.toml | grep -c 'game-player'
expected: 1, 0
actual: 1, 0
verdict: PASS
```

### Item 12 — Step reports and ledger

```text
command: git ls-files 'implementation-artifacts/strategy-discovery-m1-[0-9][0-9]-*-report.md' | wc -l
expected: 23
actual: 23

command: git ls-files 'implementation-artifacts/strategy-discovery-m1-[0-9][0-9]-*.md' | grep -vc -- '-report.md$'
expected: 24
actual: 24

command: grep -cE '^\| [0-9]{2}-[a-z0-9-]+ \| [1-4] \| done \|' implementation-artifacts/strategy-discovery-m1-ledger.md
expected: >= 20
actual: 23
verdict: PASS
```

## Phase 4 Definition of Done

### Item 13 — Throughput md/json agreement

```text
command: grep -c "^| serial | 1 | 280 | .* | <tournament_bench[0].median_games_per_s> | " artifacts/benchmarks/tournament-throughput.md
expected: 1
actual: median_games_per_s=268.1; grep count=1

command: grep -c "^| serial | 1 | 4200 | .* | <evaluate_cli[0].median_wall_s> | " artifacts/benchmarks/tournament-throughput.md
expected: 1
actual: median_wall_s=6.991; grep count=1

command: grep -cE '^\| (serial|pool|threads) \| (1|global|2|4|8) \| 280 \| ' artifacts/benchmarks/tournament-throughput.md
expected: 5
actual: 5

command: grep -cE '^\| (serial|threads-1|threads-2|threads-4|threads-8|pool) \| (1|2|4|8|16) \| 4200 \| ' artifacts/benchmarks/tournament-throughput.md
expected: 6
actual: 6
verdict: PASS
```

### Item 14 — Gate C is a pure addition below Gate B

```text
command: git diff dcd22b1 -U0 -- docs/component-evaluation.md | grep -c '^@@ -156,0 +157,'
expected: 1
actual: 1

command: git diff dcd22b1 -- docs/component-evaluation.md | grep -cE '^-[^-]'
expected: 0
actual: 0

command: grep -c '^Gate B verdict: PASS$' docs/component-evaluation.md
expected: 1
actual: 1

command: sed -n '/^## Gate C$/,$p' docs/component-evaluation.md | grep -cE '^\| [1-5] \| .+ \| (PASS|FAIL) \| .+ \|$'
expected: 5
actual: 5

command: sed -n '/^## Gate C$/,$p' docs/component-evaluation.md | grep -c '| FAIL |'
expected: 0
actual: 0
verdict: PASS
```

Overall verdict: PASS
