# ZPL Test Corpus Expansion Plan

> **Status:** Planning  
> **Created:** 2026-02-08  
> **Owner:** Core Team

---

## Executive Summary

Expand the ZPL test corpus from 5 samples (141 lines) to 30-50 curated real-world and synthetic labels to improve validation coverage, catch edge cases, and build confidence in real-world compatibility—while avoiding licensing and PII issues through a hybrid curated + synthetic approach.

---

## Current State

### Test Coverage (As of v0.1.0)

- ✅ **11 golden snapshot tests** — parser/validator AST output across key scenarios
- ✅ **5 real-world samples** (141 lines total):
  - `usps_surepost_sample.zpl` (54 lines) — complex shipping label
  - `compliance_label.zpl` (42 lines) — GS1 compliance with SSCC/GTIN
  - `shipping_label.zpl` (18 lines) — basic shipping
  - `product_label.zpl` (16 lines) — product info
  - `warehouse_label.zpl` (11 lines) — warehouse tracking
- ✅ **393+ passing tests** — parser, validator, emitter, fuzz smoke tests, print client
- ✅ **100% command coverage** — 223/223 ZPL II commands across 216 spec files

### Testing Infrastructure

- `crates/core/tests/samples.rs` — `lint_samples_directory()` validates all `samples/*.zpl` files
- `crates/core/tests/snapshots.rs` — golden AST/diagnostic snapshots with `UPDATE_GOLDEN=1` regeneration
- `crates/core/tests/fuzz_smoke.rs` — adversarial input testing (random bytes, pathological patterns)

### Current Gaps

1. **Limited diversity** — 5 samples don't exercise the full command space or real-world label complexity
2. **No stress testing** — largest sample is 54 lines; production labels can be 500+ lines or multi-label batches
3. **Missing edge cases** — unusual command combinations, obscure parameters, complex state transitions
4. **Performance unknowns** — no benchmarking against large/complex labels

---

## Goals

### Primary Goals

1. **Increase real-world confidence** — validate parser/validator against diverse production labels
2. **Catch edge cases** — discover spec interpretation issues before users do
3. **Regression safety** — prevent future refactoring from breaking real-world compatibility
4. **Performance baseline** — establish benchmarks for parser/validator performance

### Non-Goals

- ❌ Maximize quantity (1000+ labels) — quality and diversity matter more
- ❌ Test every possible ZPL permutation — infeasible and unnecessary
- ❌ Replace existing unit/integration tests — corpus is supplementary

---

## Acquisition Strategy

### Phase 1: Curated Real-World Labels (Target: 15-20 labels)

**Sources** (manually inspected, PII-scrubbed):

1. **Neodynamic ZPL Printer Emulator SDK** — samples directory with diverse label types
2. **ZebraDevs SmartKiosk Print Demo** — app-grade labels (label1.zpl, label2.zpl, template.zpl)
3. **BISG Common Carrier 4x6 shipping labels** — complex, gnarly shipping labels with barcodes/zones
4. **Virtual ZPL Printer test harness** — known-working examples from test workflows
5. **Hand-picked GitHub gists** — visually inspect 10-15 high-quality gists (US shipping labels, compliance labels)

**Selection Criteria:**

- ✅ Diverse command usage (graphics, multiple barcode types, fonts, state changes)
- ✅ Realistic complexity (50-200 lines, multi-field, multi-label)
- ✅ No obvious PII (scrub addresses/phone numbers/tracking IDs before committing)
- ✅ Public domain or permissive licensing (document source + license)
- ✅ Parses cleanly with current toolchain (validates our spec accuracy)

**Acquisition Process:**

1. Download candidate labels to `corpus/third-party-review/` (gitignored)
2. Run through parser/validator to verify compatibility
3. Scrub PII (replace with synthetic data: "123 MAIN ST" → "123 TEST STREET")
4. Document source URL + license in `corpus/sources.json`
5. Commit scrubbed label to `corpus/real-world/`

### Phase 2: Synthetic Label Generator (Target: 15-30 labels)

**Why synthetic?**

- ✅ No licensing/PII concerns
- ✅ Targeted coverage (exercise specific commands, edge cases, constraints)
- ✅ Aligns with spec-first philosophy
- ✅ Reproducible and maintainable

**Generator Design:**

- **Location:** `crates/corpus-generator/` (new workspace crate)
- **Input:** Parser tables (`generated/parser_tables.json`) + templates
- **Output:** Synthetic `.zpl` files in `corpus/synthetic/`
- **Capabilities:**
  - Generate labels exercising all 223 commands
  - Boundary value testing (min/max args, profile bounds)
  - Cross-command state transitions (`^BY`→barcodes, `^CF`→`^A`, `^FW`→orientations)
  - Constraint validation (requires/incompatible, order, emptyData)
  - Stress testing (500+ line labels, 100-label batches)

**Generator Categories:**

1. **Command coverage** — one label per command family (barcodes, graphics, fonts, etc.)
2. **Edge cases** — boundary values, empty args, max arity, prefix changes
3. **State machines** — complex cross-command state flows
4. **Stress tests** — large labels, deeply nested fields, multi-label batches
5. **Profile-specific** — labels targeting specific printer capabilities (RFID, cutter, etc.)

**Implementation Plan:**

```rust
// crates/corpus-generator/src/main.rs
pub struct LabelBuilder {
    commands: Vec<Command>,
    state: LabelState,
}

impl LabelBuilder {
    pub fn barcode_comprehensive() -> Self { /* ... */ }
    pub fn graphics_stress() -> Self { /* ... */ }
    pub fn state_machine_complex() -> Self { /* ... */ }
    pub fn to_zpl(&self) -> String { /* ... */ }
}
```

---

## Storage and Organization

### Directory Structure

```
corpus/
├── README.md                      # Corpus overview + usage instructions
├── sources.json                   # Metadata: source URLs, licenses, hashes
├── real-world/                    # Curated, scrubbed production labels
│   ├── shipping/
│   │   ├── fedex_4x6.zpl
│   │   ├── ups_surepost.zpl
│   │   └── usps_priority.zpl
│   ├── compliance/
│   │   ├── gs1_sscc18.zpl
│   │   ├── fda_pharma.zpl
│   │   └── hazmat.zpl
│   ├── industrial/
│   │   ├── warehouse_pallet.zpl
│   │   ├── inventory_bin.zpl
│   │   └── asset_tracking.zpl
│   └── retail/
│       ├── price_tag.zpl
│       ├── shelf_label.zpl
│       └── receipt.zpl
├── synthetic/                     # Generated labels (committed)
│   ├── barcode_comprehensive.zpl
│   ├── graphics_stress.zpl
│   ├── state_machine_complex.zpl
│   ├── boundary_values.zpl
│   └── profile_rfid.zpl
└── third-party-review/            # Temp staging area (gitignored)
    └── .gitkeep
```

### `sources.json` Schema

```jsonc
{
  "version": "1.0.0",
  "labels": [
    {
      "filename": "real-world/shipping/fedex_4x6.zpl",
      "source": "https://gist.github.com/user/abc123",
      "license": "MIT",
      "date_acquired": "2026-02-08",
      "sha256": "...",
      "modifications": ["PII scrubbed: addresses, tracking numbers"],
      "notes": "FedEx 4x6 shipping label with Code 128 and QR code"
    }
  ]
}
```

### `.gitignore` Additions

```gitignore
# Corpus staging (not for commit)
corpus/third-party-review/*
!corpus/third-party-review/.gitkeep
```

---

## Testing Integration

### 1. Extend `samples.rs` Test

```rust
// crates/core/tests/samples.rs

#[test]
fn lint_samples_directory() {
    // existing test (samples/*.zpl)
}

#[test]
fn lint_corpus_real_world() {
    let corpus_dir = root.join("corpus/real-world");
    if !corpus_dir.exists() {
        eprintln!("Skipping corpus test (corpus/real-world not found)");
        return;
    }
    // recursively lint all .zpl files in corpus/real-world/
}

#[test]
fn lint_corpus_synthetic() {
    let corpus_dir = root.join("corpus/synthetic");
    if !corpus_dir.exists() {
        eprintln!("Skipping synthetic corpus test (corpus/synthetic not found)");
        return;
    }
    // recursively lint all .zpl files in corpus/synthetic/
}
```

### 2. Add Corpus Stress Test

```rust
// crates/core/tests/corpus_stress.rs (new file)

#[test]
fn corpus_parse_all_labels_cleanly() {
    // Parse all corpus labels, assert zero errors
    // (warnings are OK, errors fail the test)
}

#[test]
fn corpus_round_trip_formatting() {
    // Parse → format → parse, assert AST equivalence
}

#[test]
fn corpus_performance_baseline() {
    // Measure parse/validate time for largest labels
    // Fail if any label takes >100ms (adjust threshold as needed)
}
```

### 3. Benchmark Integration

```rust
// benches/corpus_benchmark.rs (new file, requires criterion)

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_corpus(c: &mut Criterion) {
    let labels = load_all_corpus_labels();
    c.bench_function("parse_corpus_all", |b| {
        b.iter(|| {
            for (name, zpl) in &labels {
                let res = parse_with_tables(black_box(zpl), Some(&TABLES));
                black_box(res);
            }
        })
    });
}

criterion_group!(benches, bench_corpus);
criterion_main!(benches);
```

### 4. CI Integration

```yaml
# .github/workflows/ci.yml

- name: Corpus tests
  run: cargo test --test samples --test corpus_stress

- name: Corpus benchmark (informational)
  run: cargo bench --bench corpus_benchmark -- --output-format bencher
  continue-on-error: true  # don't fail CI on perf regression (yet)
```

---

## Implementation Plan

### Phase 1: Foundation (Week 1)

- [x] Draft planning document (this file)
- [ ] Create `corpus/` directory structure
- [ ] Add `corpus/README.md` with usage instructions
- [ ] Add `corpus/sources.json` schema
- [ ] Update `.gitignore` for `third-party-review/`
- [ ] Extend `samples.rs` test to include corpus directories

### Phase 2: Curated Acquisition (Week 2)

- [ ] Download 20-30 candidate labels from sources (to `third-party-review/`)
- [ ] Lint candidates with current toolchain (identify incompatibilities)
- [ ] Select 15-20 best labels (diverse, realistic, compatible)
- [ ] Scrub PII from selected labels
- [ ] Document sources in `sources.json`
- [ ] Commit scrubbed labels to `corpus/real-world/`
- [ ] Verify `lint_corpus_real_world` test passes

### Phase 3: Synthetic Generator (Week 3-4)

- [ ] Create `crates/corpus-generator/` workspace crate
- [ ] Implement `LabelBuilder` API (command chaining, state tracking)
- [ ] Generate 5 initial synthetic labels (one per category)
- [ ] Validate synthetic labels parse/lint cleanly
- [ ] Generate remaining 10-25 synthetic labels (edge cases, stress tests)
- [ ] Commit synthetic labels to `corpus/synthetic/`
- [ ] Verify `lint_corpus_synthetic` test passes

### Phase 4: Testing & Benchmarks (Week 5)

- [ ] Implement `corpus_stress.rs` tests (parse all, round-trip, performance)
- [ ] Add `corpus_benchmark.rs` criterion benchmarks
- [ ] Run full test suite with corpus (identify regressions)
- [ ] Document any parser/validator issues found
- [ ] Fix critical issues (defer minor edge cases to backlog)
- [ ] Update CI workflow to include corpus tests

### Phase 5: Documentation & Release (Week 6)

- [ ] Update `CONTRIBUTING.md` with corpus contribution guidelines
- [ ] Add corpus section to root `README.md`
- [ ] Document any spec changes discovered during corpus expansion
- [ ] Tag corpus expansion completion in `CHANGELOG.md`
- [ ] Announce corpus expansion in release notes

---

## Success Metrics

### Quantitative

- ✅ **30-50 total labels** in corpus (15-20 real-world + 15-30 synthetic)
- ✅ **>90% command coverage** across corpus (measured by `coverage.json`)
- ✅ **Zero parser errors** across entire corpus
- ✅ **<5 validator warnings** across corpus (excluding intentional edge cases)
- ✅ **<100ms parse time** for any single label (95th percentile)

### Qualitative

- ✅ **Diverse command usage** — all major command families represented
- ✅ **Realistic complexity** — labels resemble production use cases
- ✅ **No licensing issues** — all sources documented, PII scrubbed
- ✅ **Maintainable** — synthetic generator makes corpus easy to extend

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **PII leakage in corpus** | Medium | High | Manual review + automated scrubbing script |
| **Licensing violations** | Medium | High | Document sources, prefer public domain/MIT |
| **Low-quality labels from GitHub** | High | Low | Manual curation, parse/lint validation |
| **Generator produces invalid ZPL** | Medium | Medium | Validate all synthetic labels with toolchain |
| **CI slowdown from large corpus** | Low | Medium | Use feature flags, run on schedule vs per-PR |
| **Corpus reveals spec bugs** | Medium | High | **Good risk!** Document and fix issues |

---

## Open Questions

1. **How many labels is "enough"?** → Start with 30-50, expand if needed
2. **Should corpus be in main repo or separate?** → Main repo (small size, tight integration)
3. **Commit binary/image data for `^GF` tests?** → Yes, but limit to <100KB per file
4. **Run corpus tests on every PR or nightly?** → Every PR for `lint_corpus_*`, nightly for benchmarks
5. **Accept community corpus contributions?** → Yes, via issue (paste + source), not direct PRs

---

## References

- [ZPL II Programming Guide (PDF)](https://www.zebra.com/us/en/support-downloads.html) — official ZPL reference
- [Neodynamic ZPL Printer Emulator](https://github.com/neodynamic/zpl-printer-emulator) — sample labels
- [BISG Common Carrier Label Generator](https://bisg.org) — shipping label templates
- [Labelary ZPL Viewer](http://labelary.com/viewer.html) — visualize ZPL output
- ChatGPT conversation (2026-02-08) — corpus acquisition strategy brainstorming

---

## Change Log

| Date | Author | Change |
|------|--------|--------|
| 2026-02-08 | AI Assistant | Initial draft |
