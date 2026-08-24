# strategy-discovery-m3 — Project gate

## Tuning

Phase 4 performed no tuning: `configs/tictactoe-concepts.toml` is unchanged since its
phase-3 creation.

- evidence: `git log -1 --format=%h -- configs/tictactoe-concepts.toml` -> `f3ad73e`
  (the 32-fixtures step commit; the ledger records its merge commit as `0d13aed`)
- evidence: `git diff 0d13aed..HEAD -- configs/tictactoe-concepts.toml` -> empty; the
  blob is `f5027447e3d9c3de5d3c45fa874b6bf93a00ea8e` at both ends, so phase 4 changed
  the fixture in no byte
- effective values: `[induction] enabled = true, withhold_tier2 = true, rounds = 2`
  (all other keys serde defaults)
- effective values: `[mine] engine = "cart", depths = [6, 8, 12, 0], min_leaf = 1`

## Project Definition of Done

### Item 1 — toolchain gates, wall budget, Cargo diff

Commands and results:

```text
cargo check --workspace --all-targets
-> Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.99s
-> exit 0

cargo fmt --all --check
-> (no output)
-> exit 0

cargo clippy --workspace --all-targets --all-features -- -D warnings
-> Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.69s
-> exit 0

cargo doc --no-deps --workspace > target/m3/gate-doc.txt 2>&1
-> exit 0
grep -c 'warning' target/m3/gate-doc.txt
-> 0

cargo test --workspace > target/m3/gate-t1-out.txt 2> target/m3/gate-t1-err.txt   (warm-up)
-> exit 0

pwsh -NoProfile -Command "Measure-Command { cargo test --workspace 2> target/m3/gate-t2-err.txt > target/m3/gate-t2-out.txt } | Select-Object -ExpandProperty TotalSeconds"
-> 51.4425599   (<= 90)
grep -c '^test result: ok' target/m3/gate-t2-out.txt
-> 28
grep -c ' 0 failed' target/m3/gate-t2-out.txt
-> 28

git diff c997c351f636bc80b7b9bde46b7aee7a309f5f20 -- Cargo.toml Cargo.lock
-> (empty)
```

- verdict: PASS

### Item 2 — concepts-1 exit/stdout/files

Evidence from `target/m3/gate-runs.ps1` (run once, covers items 2-7 and the repeat run):

```text
CHECK exit-0: PASS exit=0
CHECK stdout-pinned: PASS lines=11
CHECK files-18: PASS files=18
```

- verdict: PASS

### Item 3 — concepts.json facts

```text
CHECK concepts-json: PASS promoted=2 withheld=22 rows=557 params_hash=f1596db3a853769f
```

- verdict: PASS

### Item 4 — tier-3-referencing candidate conjunction

```text
CHECK conjunction: PASS qualifying=mined-d12-l1,mined-d0-l1 d0-refs=concept_2
conjunction=PASS (mined-d12-l1, mined-d0-l1)
```

The conjunction holds per archive entry: both qualifying loss-0 tier-3-referencing
candidates (mined-d12-l1, mined-d0-l1) reference concept_2.

- verdict: PASS

### Item 5 — report.md

```text
CHECK report-md: PASS d0-tier3-def-lines=2
```

- verdict: PASS

### Item 6 — dataset vocabulary

```text
CHECK dataset-vocabulary: PASS columns=82 invented_fraction=0.0196078431372549

grep -c '^## Vocabulary$' target/m3/concepts-1/report.md
-> 1
```

- verdict: PASS

### Item 7 — repeat-run determinism

```text
CHECK repeat-identical: PASS files=18
hash concepts.json 541c5ea59ea43739f8fc8da969499c5cc3d8193d3f1d694f0150b6ac52bb71ac
hash evaluation.json 0e9c3423b5ef240316f6f0567fbb5a52897c78c46f562bba99e766ec63fdc153
hash heuristics.json c52ba9bf7d150837fdc11a0d383293fea3292f98853aac22abe0db017aad13a6
hash report.md e78d8a1509e25a088608a3cbfa67328d67b095af0ea404e6b7df18684a2b0765
CHECK hashes: PASS
RESULT: ALL CHECKS PASS

grep -c 'identical: YES' artifacts/reproducibility/concept-induction-determinism.md
-> 2
grep -c 'identical: NO' artifacts/reproducibility/concept-induction-determinism.md
-> 0
grep -Ec '^\| [A-Za-z0-9./_-]+ \| [0-9a-f]{64} \| [0-9a-f]{64} \|$' artifacts/reproducibility/concept-induction-determinism.md
-> 18
```

- verdict: PASS

### Item 8 — promotion-criteria tests

```text
cargo test rejects_candidate > target/m3/gate-rej.txt 2>&1
-> exit 0

grep -c 'rejects_candidate_without_holdout_gain ... ok' target/m3/gate-rej.txt
-> 1
grep -c 'rejects_candidate_without_downstream_improvement ... ok' target/m3/gate-rej.txt
-> 1
grep -c 'test result: ok. 2 passed' target/m3/gate-rej.txt
-> 1
```

- verdict: PASS

### Item 9 — stage independence

```text
cargo test --test cli_concepts > target/m3/gate-concepts.txt 2>&1
-> exit 0
-> test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.15s

./target/debug/strategy-discovery.exe analyze --list-analyzers > target/m3/gate-la.txt 2>/dev/null
wc -l < target/m3/gate-la.txt
-> 6
cut -f1 target/m3/gate-la.txt | paste -sd, -
-> agreement,concepts,dataset,mine,summary,vocabulary
```

- verdict: PASS

### Item 10 — regression

```text
./target/debug/strategy-discovery.exe annotate --exhaustive --game tictactoe --out target/m3/p1-annotate > target/m3/gate-ann.txt 2>&1
-> exit 0
-> mode=exhaustive annotated=5478 terminal=958 disagreements=0

./target/debug/strategy-discovery.exe evaluate --game tictactoe --strategies tests/fixtures/strategies-eval.toml --annotations target/m3/p1-annotate --out target/m3/p1-eval > target/m3/gate-eval.txt 2>&1
-> exit 0

sha256sum target/m3/p1-eval/evaluation.json target/m3/p1-eval/archive/archive.json target/m3/p1-eval/archive/entries.jsonl
-> 1487b18bd8cdb5efd900a099766353aa8f5b1a09d96c9412717b8df4e6dcce0a *target/m3/p1-eval/evaluation.json
-> 6834c0e340e8e8ea5bb9af079beb650444425d27d6c06a7c252bb7ca72cd330a *target/m3/p1-eval/archive/archive.json
-> 8f5a1bd2775294a6f3718afeb0a6330726a043cb22d318dd92f47330d4d2d5d8 *target/m3/p1-eval/archive/entries.jsonl
(matches all three M1 pins, in order)

cargo test --test cli_pipeline --test corpus --test cli_experiment --test cli_discover --test dataset --test mine > target/m3/gate-reg.txt 2>&1
-> exit 0
grep -c '^test result: ok' target/m3/gate-reg.txt
-> 6
grep -c ' 0 failed' target/m3/gate-reg.txt
-> 6

pwsh -NoProfile -ExecutionPolicy Bypass -File target/m3/m2-check.ps1
-> m2 fresh run captured.

sed -n '/^run_id=bd760ff0ace48705 cells=48 games=1920 positions=14887 out=target\/m2\/p3-discover$/,/^discover=tictactoe-discover /p' README.md > target/m3/m2-readme-block.txt
diff target/m3/m2-fresh-stdout.txt target/m3/m2-readme-block.txt && echo M2-BLOCK-DIFF-EMPTY
-> M2-BLOCK-DIFF-EMPTY

git diff --stat c997c351f636bc80b7b9bde46b7aee7a309f5f20 -- docs/plan.md .github docs/component-evaluation.md docs/adding-a-game.md tests/invariants.rs configs/tictactoe-default.toml configs/tictactoe-experiment.toml configs/tictactoe-discover.toml 'docs/adr/000[1-9]-*.md' 'docs/adr/001[0-6]-*.md' 'tests/fixtures/generate-*.toml' tests/fixtures/experiment-small.toml tests/fixtures/discover-small.toml tests/fixtures/roster-tiny.toml 'tests/fixtures/strategies-*.toml' artifacts/benchmarks/annotation-per-position.json artifacts/benchmarks/annotation-per-position.md artifacts/benchmarks/corpus-generation.json artifacts/benchmarks/corpus-generation.md artifacts/benchmarks/mining-induction.json artifacts/benchmarks/mining-induction.md artifacts/benchmarks/record-format-compile.json artifacts/benchmarks/record-format.json artifacts/benchmarks/record-format.md artifacts/benchmarks/rule-induction.json artifacts/benchmarks/rule-induction.md artifacts/benchmarks/selfplay-throughput.json artifacts/benchmarks/selfplay-throughput.md artifacts/benchmarks/tournament-throughput.json artifacts/benchmarks/tournament-throughput.md artifacts/reproducibility/annotation-determinism.md artifacts/reproducibility/cli-outputs-determinism.md artifacts/reproducibility/corpus-determinism.md artifacts/reproducibility/corpus-diversity.md artifacts/reproducibility/discover-determinism.md artifacts/reproducibility/selfplay-serial-vs-parallel.md artifacts/reproducibility/tournament-determinism.md
-> (no output)
```

- verdict: PASS

### Item 11 — docs and neutrality

```text
head -1 docs/adr/0017-mechanical-tier1-extension.md
-> # ADR 0017: Mechanical tier-1 extension
grep -c '^## ' docs/adr/0017-mechanical-tier1-extension.md
-> 4
grep -c 'Milestone 3 / plan Phase 9' docs/adr/0017-mechanical-tier1-extension.md
-> 1
heading-blank-line awk -> exit 0
grep -c -- '|---' docs/adr/0017-mechanical-tier1-extension.md
-> 0

head -1 docs/adr/0018-concept-induction.md
-> # ADR 0018: Concept induction
grep -c '^## ' docs/adr/0018-concept-induction.md
-> 4
grep -c 'Milestone 3 / plan Phase 9' docs/adr/0018-concept-induction.md
-> 1
heading-blank-line awk -> exit 0
grep -c -- '|---' docs/adr/0018-concept-induction.md
-> 0

grep -c '^#### Readability example$' README.md
-> 1
grep -c '^enabled = false' README.md
-> 1
grep -c '(label: last open corner)$' README.md
-> 1
grep -c '| `concepts.json` | analyze,' README.md
-> 1
grep -c '| `vocabulary.json` | analyze,' README.md
-> 1
heading-blank-line awk on README.md -> exit 0

grep -c 'concept induction' CLAUDE.md
-> 1

v=0; for f in $(find src/core src/discovery src/io src/cli -name '*.rs' ! -path '*games.rs'); do n=$(awk '/#\[cfg\(test\)\]/{exit} {print}' "$f" | grep -c -i 'tictactoe'); if [ "$n" != "0" ]; then echo "$f: $n"; v=1; fi; done; echo "game_name_violations=$v"
-> game_name_violations=0

v=0; for f in $(find src/core src/discovery src/io src/cli -name '*.rs' ! -path '*games.rs'); do n=$(awk '/#\[cfg\(test\)\]/{exit} {print}' "$f" | grep -c -E '\b(threat|fork|corner|center|edge)\b'); if [ "$n" != "0" ]; then echo "$f: $n"; v=1; fi; done; echo "concept_violations=$v"
-> concept_violations=0
```

- verdict: PASS

### Item 12 — benchmark artifacts and bench vehicle

```text
grep -c '| build_profile | release |' artifacts/benchmarks/concept-induction.md
-> 1
grep -c '| mined-d0-l1 | 0 | 173 | 175 | 3 | 0.000 | 0.993 | 0.031 |' artifacts/benchmarks/concept-induction.md
-> 1

pwsh -NoProfile -Command "... ConvertFrom-Json ... header/full_scale/generate_small field checks ..."
-> json-ok

cargo test --test induction_bench -- --nocapture > target/m3/gate-bench.txt 2>&1
-> exit 0
grep -c '^\[bench\] ' target/m3/gate-bench.txt
-> 5
grep -c 'test result: ok. 1 passed' target/m3/gate-bench.txt
-> 1
```

- verdict: PASS

### Item 13 — process

```text
git ls-files 'implementation-artifacts/strategy-discovery-m3-4*-report.md' | wc -l
-> 5
  (41-verify, 42-config, 43-determinism, 44-benchmark, 45-readme; this step's own
   report, 46-gate, is committed in the same commit that carries this gate report,
   so it is not yet present at grep time)

grep -c ' | 4 | done | ' implementation-artifacts/strategy-discovery-m3-ledger.md
-> 5
```

- verdict: PASS

Overall verdict: PASS
