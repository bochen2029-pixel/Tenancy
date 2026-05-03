# Ground Truth Framework — Closed-Loop State Diagram

State diagram representation of §4.7 closed-loop reflexivity dynamics.
The framework operates as a closed-loop dynamical system: classification IS the
perturbation event; observer's response feeds back into next classification update.

Reference: framework.md §4.7, Eq (21), reference_impl/closed_loop.py

---

## State Variables

| Variable | Domain | Meaning |
|---|---|---|
| `q(t)` | [0, 1] | Observer's posterior probability of S->C trajectory |
| `g(q)` | [0, 1] | Access-grant function: how much access observer extends at posterior q |
| `r(g)` | [0, 1] | Subject's response under access level g |
| `eta` | (0, 1) | Observer's update rate per observation |

---

## Dynamics (Eq 21)

```
q(t + dt) = q(t) + eta * [observation(r(g(q(t)))) - prediction(q(t))]
```

Where:
- `observation` = subject's observed response to access level
- `prediction` = `g(q)` under "if classification correct, response should match access"
- The bracket is the Bayesian innovation: observed-minus-predicted response

---

## Fixed Points

```
                    q* = 1 (committed)
                    /
                   /  Genuine S-mode subjects produce r(g(1)) = 1
                  /   observation matches prediction
                 /    bracket -> 0
                /     posterior stays at 1 (locally stable)
   q ∈ (0,1) --+
                \
                 \    E-mode-with-cover produces r(g(0)) ≈ 0
                  \   no access granted, no extraction available
                   \  bracket -> 0
                    \ posterior stays at 0 (locally stable)
                    \
                    q* = 0 (rejected)
```

---

## State Diagram (ASCII)

```
                                    ┌─────────────────┐
                                    │                 │
                                    │  COMMITTED      │
                          ┌────────►│  q* = 1         │◄──── CONVERGENCE
                          │         │                 │      r(g) → 1 as g → 1
                          │         └─────────────────┘
                          │                 ▲
                          │                 │
                ┌─────────┴─────────────────┼─────────┐
                │                           │         │
                │      INTERMEDIATE         │         │
                │   q ∈ (0, 1) transient    │         │
                │                           │         │
                │   Bayesian update:        │         │
                │   q ← q + eta * [r(g(q)) - g(q)]    │
                │                           │         │
                └─────────┬─────────────────┼─────────┘
                          │                 │
                          │                 │
                          │         ┌─────────────────┐
                          │         │                 │
                          └────────►│  REJECTED       │
                                    │  q* = 0         │
                                    │                 │
                                    └─────────────────┘
                                            ▲
                                            │
                                       CONVERGENCE
                                       r(g) → 0 as g → 1
```

---

## Failure Modes (Diagrammed)

### FM-CL-01: Premature Access-Grant

```
                          q          0.3        0.5         0.7         1.0
                          ├───────────┼───────────┼───────────┼───────────┤
   STANDARD g(q):                    ▁▁▁▁▁▁▁▁▁▁▁ ▃▃▃▃▃▃▃▃▃▃▃ █████████████
   PREMATURE g(q):       █████████████████████████████████████████████████ ← FAILURE

   Premature: g(q) jumps to high access at low q
   Result: E-mode-with-cover extracts before bracket converges
   Mitigation: g(q) <= 0.5 for q < 0.7
```

### FM-CL-02: Update Rate Too Low (eta small)

```
   q(t)
   1.0 │
       │
   0.7 │                                                  ╭──── slow convergence
       │                                            ╭─────╯     (CDS subsidizes
   0.5 ┝─────────────────────────╮    ╭─────────────╯           continued cover)
       │                         ╰────╯
   0.3 │
       │
   0.0 ┕──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────►
              0      6     12     18     24     30     36     time (months)

   Long convergence window allows cover-performance to be subsidized via cross-dyad
   extraction. Detection: q not converging within ~50 observations.
   Mitigation: increase eta; apply Protocol F (cross-dyad observation).
```

### FM-CL-03: Update Rate Too High (eta large) — Oscillation

```
   q(t)
   1.0 │       ╱╲              ╱╲              ╱╲
       │      ╱  ╲            ╱  ╲            ╱  ╲
   0.7 │     ╱    ╲          ╱    ╲          ╱    ╲
       │    ╱      ╲        ╱      ╲        ╱      ╲
   0.5 ┝───╯        ╲──────╱        ╲──────╱        ╲──────► oscillating
       │            ╲      ╱         ╲      ╱
   0.3 │             ╲    ╱           ╲    ╱
       │              ╲  ╱             ╲  ╱
   0.0 │               ╲╱               ╲╱
       └──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────►
              0      1      2      3      4      5      6     time

   Single-event noise dominates updates; |dq| > 0.3 sustained over 3+ updates.
   Detection: oscillation per is_oscillating().
   Mitigation: reduce eta by factor of 2; smooth observation window.
```

---

## Convergence Trajectories (Healthy Operation)

### Genuine S-mode → C convergence

```
   q(t)
   1.0 │                                              ╭─────────── q* = 1
       │                                       ╭──────╯
   0.7 │                                ╭──────╯
       │                          ╭─────╯
   0.5 │                    ╭─────╯
       │              ╭─────╯
   0.3 ┝──────────────╯
       │
   0.0 │
       └──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────►
              0      3      6      9     12     15     18     time (months)

   Subject in genuine S-mode; Phi_global recovery proceeds per Eq (16);
   q updates upward as F13a kappa_bid < 0, F14a depth progresses, etc.
   Convergence to committed fixed point in expected window.
```

### E-mode-with-cover → rejected convergence

```
   q(t)
   1.0 │
       │
   0.7 │
       │
   0.5 ┝──────╮
       │      ╲
   0.3 │       ╲────╮
       │            ╲────╮
   0.0 │                 ╲──────────╮─────────── q* = 0
       │                            ╲────────────────────────
       └──────┬──────┬──────┬──────┬──────┬──────┬──────┬──────►
              0      3      6      9     12     15     18     time (months)

   Subject in E-mode-with-cover; observer's standard g(q) does not grant high access
   at low q; subject's response under low access reveals cover-performance not authentic;
   F1 covariance becomes detectable; F10 spike under any access-grant attempt.
   Convergence to rejected fixed point.
```

---

## Mutual-S Configuration (Outside Framework Reach)

```
   Observer's q     ↑
                    │
                  1 │ ⟶  forbidden region
                    │      (both wait for evidence)
                  0.7│                               ╲
                    │                                 ╲
                  0.5│  ◀──────── stuck at indeterminate ────────▶
                    │                                 ╲
                  0.3│                                  ╲
                    │
                  0 │
                    └────┬──────┬──────┬──────┬──────►
                         0    0.3   0.5    0.7    1   Subject's q
   
   Mutual-S: both parties' q stuck at intermediate values.
   Schelling problem: someone must demonstrate C-disposition first,
   but both wait for evidence first.
   FRAMEWORK CANNOT RESOLVE; deferred to v4.0 coordination layer.
```

---

## Practical Implications

1. **Observer must calibrate `g(q)` appropriately.** Premature access-grant at low q is the most common framework failure. Default: `g(q) = 0.05` for `q < 0.3`, ramping to `0.5` at `q = 0.7`, then to `1.0` at `q = 1.0`.

2. **Update rate `eta` should be tuned.** Default: `eta = 0.1`. Lower for high-noise observation conditions; higher for high-information observation conditions. Monitor for oscillation (eta too high) or slow convergence (eta too low).

3. **The closed-loop is the verification.** The framework is gaming-resistant *at the resolution event* per §4.7. Genuine S-mode resolves to commitment when access is granted; E-mode-with-cover triggers extraction. The middle case (sustained gaming that never cashes in) collapses to authenticity under Axiom 2 - if behavior is lifetime-indistinguishable, the system is authentic for decision-theoretic purposes.

4. **Observer awareness of dynamics matters.** A framework-aware observer who calibrates `g(q)` and `eta` correctly will achieve faster, more reliable convergence than an observer who runs the framework as a static classifier without closed-loop awareness.

---

## Reference Implementation

See `reference_impl/closed_loop.py`:
- `simulate_closed_loop()` runs the dynamics
- `standard_access_grant()` implements default `g(q)`
- `genuine_s_mode_response()` and `e_mode_with_cover_response()` model subject behavior
- `detect_premature_access_grant()`, `detect_update_rate_too_low()`, `detect_update_rate_too_high()` flag failure modes
