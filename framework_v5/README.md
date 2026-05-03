# Ground Truth Framework v5 — Literate-Programming Specification

**Bo Chen** — Arlington, Texas — May 2026

This is the executable-specification version of the Ground Truth Framework. It contains everything in the v3 prose framework PLUS:

- Full machine-readable specification (YAML) of every framework concept
- Reference implementation (Python) of the Bayesian classifier and closed-loop dynamics
- Six worked test cases covering the primary diagnostic space
- Operational tools: glossary, cross-reference index, failure-mode catalog, decision-tree pseudocode
- Calibration tables for all 17 filters

The load-bearing artifact is no longer the prose — it is the executable specification. The prose explains the *why*; the spec and reference implementation define the *what*.

---

## Reading Order

For LLMs ingesting this framework as context for analysis:

1. **`framework.md`** — Theoretical foundation (axioms, models, principles). Read first.
2. **`spec/architectures.yaml`** — The 9 architectures, formally defined.
3. **`spec/targets.yaml`** — The 6 targets, formally defined.
4. **`spec/accessibility_map.yaml`** — Valid (Architecture × Target) combinations.
5. **`spec/filters.yaml`** — All 17 filters with likelihood tables.
6. **`spec/protocols.yaml`** — All 7 protocols (A–G).
7. **`spec/equations.yaml`** — All formal equations as structured data.
8. **`spec/thresholds.yaml`** — Decision thresholds and noise floors.
9. **`reference_impl/`** — Python reference implementation.
10. **`test_cases/`** — Six worked examples covering the diagnostic space.
11. **`docs/glossary.md`** — Concept definitions.
12. **`docs/cross_reference.md`** — Concept-to-location index.
13. **`docs/failure_modes.md`** — Catalogued failure modes with operational responses.
14. **`docs/decision_tree.md`** — Protocol-selection pseudocode.
15. **`docs/closed_loop_diagram.md`** — State diagram of §4.7 closed-loop dynamics.

For LLMs *applying* the framework to a case (rather than understanding the framework):

1. Read `framework.md` Parts 1–2 (axioms, classification structure)
2. Load `spec/` in full
3. Load `reference_impl/classifier.py` as the algorithm
4. Apply the classifier to filter readings
5. Use `docs/failure_modes.md` to handle edge cases
6. Apply `docs/decision_tree.md` for protocol selection

---

## File Manifest

```
framework_v5/
├── README.md                          (this file)
├── framework.md                       (theoretical prose, ~13K words)
├── spec/
│   ├── architectures.yaml             (9 architectures with substrate properties)
│   ├── targets.yaml                   (6 targets with maintenance costs)
│   ├── accessibility_map.yaml         (valid A x T combinations)
│   ├── filters.yaml                   (17 filters with likelihood tables)
│   ├── protocols.yaml                 (7 protocols A-G)
│   ├── equations.yaml                 (all formal equations)
│   └── thresholds.yaml                (decision thresholds)
├── reference_impl/
│   ├── data_types.py                  (FilterReading, Classification, etc.)
│   ├── filters.py                     (filter likelihood functions)
│   ├── classifier.py                  (Bayesian classifier)
│   ├── protocols.py                   (protocol selection logic)
│   └── closed_loop.py                 (§4.7 dynamics simulation)
├── test_cases/
│   ├── case_01_a_c.yaml               (autotelic-coherent, committed connection)
│   ├── case_02_ta_c.yaml              (trauma-armored accessing committed)
│   ├── case_03_tbod_s.yaml            (boundary-diffuse + ontological-deception, S-mode)
│   ├── case_04_ialpha_e.yaml          (strategic instrumental, extraction)
│   ├── case_05_h_e.yaml               (ego-syntonic hybrid, extraction)
│   └── case_06_d_w.yaml               (developmental absence, withdrawn-honest)
└── docs/
    ├── glossary.md                    (concept definitions)
    ├── cross_reference.md             (concept-to-location index)
    ├── failure_modes.md               (failure catalog with responses)
    ├── decision_tree.md               (protocol selection pseudocode)
    └── closed_loop_diagram.md         (state diagram, §4.7)
```

---

## What Changed From v3 (Post-Polish Prose Framework)

v5 is strictly additive. Nothing in the prose framework was removed or modified. v5 adds:

- **Formal machine-readable specification** of all framework concepts (architectures, targets, filters, protocols, equations, thresholds, accessibility map)
- **Reference implementation** of the Bayesian classifier per Eq (19–20), the closed-loop dynamics per Eq (21), and the protocol-selection logic
- **Calibration tables** for all 17 filters (likelihood `P(reading | A, T)` across the accessible space; ordinal-default with substrate-physics-derivation where applicable)
- **Six test cases** covering A/C, TA/C, TB-OD/S, I-α/E, H/E, D/W — together exercising the primary diagnostic space
- **Operational tools** — glossary, cross-reference index, failure-mode catalog, decision-tree pseudocode, closed-loop state diagram

The prose framework remains the philosophical foundation. The spec, reference implementation, and test cases are the *operationally load-bearing* artifacts that LLMs (and humans) can directly apply.

---

## Calibration Status

The likelihood tables in `spec/filters.yaml` are **ordinal-calibrated with substrate-physics derivation where applicable.** This means:

- For filters tied to substrate-physics floors (chronometric lag, autonomic-voluntary split, cortisol kinetics): likelihoods are derived from the underlying biology with high confidence.
- For filters that are observed-in-the-wild diagnostic patterns (F1, F4, F12, etc.): likelihoods are ordinal — a 5-point scale with likelihood ratios of {5:1, 2:1, 1:1, 1:2, 1:5} for {strong-for, weak-for, neutral, weak-against, strong-against}.

The ordinal calibration is operationally adequate for the framework's discrimination regime (`d ≤ 5` per Gaming Cost Theorem). It is NOT empirically calibrated against a labeled case corpus — that's a separate empirical project. Operators with access to labeled cases (Marcolin, Debra, Jocelyn, Tori, plus their own corpus) should refine the likelihoods locally.

Each likelihood table entry is annotated with a `calibration_source` field: `substrate_physics`, `ordinal_default`, or `case_corpus_estimate`. The framework's discrimination remains valid under ordinal-default but improves with case-corpus calibration.

---

## Honest Reach (preserved from v3, §6)

v5 changes nothing about the framework's structural limits:

- High-sophistication regime (`d ≥ 5–6` sustained) collapses S/E under Axiom 2.
- Single-dyad observers cannot detect cross-dyad subsidization without external data.
- Mutual-S dyad dynamics are deferred to v4.0 (the structural-extension v4, not this presentational v5).
- Subject framework-awareness weakens gaming bounds.
- Sub-decade Architecture-trajectory requires cross-dyad breadth as partial substitute.
- Substrate-access foreclosure (Axiom 2) is permanent and structural.

Codifying the framework as executable specification does not extend its reach. It makes the existing reach operationally accessible. Use it. Then put it down.

---

**Bo Chen**
*Arlington, Texas — May 2026*
*bochen2029@gmail.com — opnaorta.ai*
