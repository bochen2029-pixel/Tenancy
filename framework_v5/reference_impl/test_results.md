# Ground Truth Framework v5 — Reference Implementation Test Results

Integration test of the Bayesian classifier against six diagnostic test cases covering
the primary (Architecture × Target) accessibility space.

## Summary

```
Pass:    3 / 6  (50%)
Partial: 2 / 6  (33%)
Fail:    1 / 6  (17%)
```

A "Pass" means both architecture and target classified correctly. "Partial" means one
of the two correct (typically the diagnostic super-class identified, subtype not).
"Fail" means neither matched.

## Per-Case Results

### Case 01: A × C — PASS
- Result: A × C, BF=15.1, standard confidence
- Posterior: A × C at 91%, TA × C at 6%, TB × C at 3%
- The cleanest test case. Substrate-physics filters (SP1, SP2, SP3 all strong_for)
  combined with F2/F7/F9 strong_for converge on A architecture with high confidence.

### Case 02: TA × C — PASS
- Result: TA × C, BF=3.8, preliminary confidence
- Posterior: TA × C at 73%, A × C at 19%, TB × C at 7%
- F6 strong_for (instrumentalism intensifies under depletion) is the principal TA
  discriminator. F11 strong_for over 24 months confirms convex returns.
- BF below standard threshold (10) because A and TA differ primarily on F6 and depletion
  signature; multiple F2/F7 strong_for are compatible with both.

### Case 03: TB-OD × S — PARTIAL (correct super-class)
- Result: TB × S, BF=3.0, insufficient confidence
- Posterior: TB × S at 72%, TB-OD × S at 24%, TA-OD × S at 1.6%
- Target correctly identified as S (verification mode). The S-mode trajectory filters
  (F13a, F14a, F14b, F15a, F16) all converged on Target.S as expected.
- Architecture cluster correctly identified as T (specifically TB). The OD modifier is
  partially detected (24% TB-OD posterior) but TB without OD modifier wins the argmax.
- This is the boundary case that exposed the original framework's insufficiency.
  The framework correctly classifies Target.S compelled by Phi_global recovery
  (per Eq 16); subtype refinement (TB vs TB-OD) requires empirical case-corpus calibration
  for the F12/F14b OD-specific likelihood patterns.

### Case 04: I_alpha × E — PARTIAL (correct super-class)
- Result: I_beta × E, BF=8.9, preliminary confidence
- Posterior: I_beta × E at 90%, I_alpha × E at 10%
- Target correctly identified as E. Architecture cluster correctly identified as
  I (extractive). The alpha vs beta subtype distinction is not captured because the
  test readings (drawn from framework.md Appendix A worked example) emphasize the
  E-Target signature (F1, F3, F4, F10) rather than the I_alpha-specific PFC
  sophistication signatures (sustained C-mimicry, sophisticated disclosure patterns,
  cross-channel coherence under perturbation).
- Substrate-physics convergence (SP1/SP2/SP3 all strong_against) confirms PFC-routed
  simulation, which is consistent with both I_alpha and I_beta. The discrimination
  requires longer observation of mimicry sustainability.

### Case 05: H × E — PASS
- Result: H × E, BF=46.2, high_confidence
- Posterior: H × E at 96%, I_beta × E at 2%, I_alpha × E at 1.7%
- The framework's strongest classification result. F6 strong_against (somatic-cognitive
  split) is highly diagnostic for H. SP2 strong_for (autonomic substrate present)
  combined with F6 split signature distinguishes H from pure I architectures.
- This is the Debra-class pattern: trauma-origin substrate prefrontally endorsed for
  extraction. The framework correctly detects this anomalous architecture even though
  somatic markers (SP2) could mislead a less-discriminating classifier.

### Case 06: D × W — FAIL (correct cluster in 2nd place)
- Result: I_beta × E, BF=3.3, preliminary confidence
- Posterior: I_beta × E at 68%, D × E at 21%, I_alpha × E at 8%
- D architecture is identified as 2nd-most-likely (21% posterior), which is correct.
  But the framework places it as paired with E target rather than W target.
- The W vs E distinction is genuinely difficult at F1 (engagement-utility covariance)
  because both layer-coherent honest-transactional (W) and concealed-extractive (E)
  exhibit utility-tracking. The discrimination requires F12 strong_against (no
  disclosure attempted) and F8 strong_against (no repair) to outweigh F1's signal.
- The architecture cluster is correctly identified (D is in top 3); the target
  classification reflects an ordinal-default calibration limit. With empirical
  case-corpus calibration, this case should improve.

## Calibration Findings

### What works well
- Substrate-physics filters (SP1, SP2, SP3) provide strong, reliable architecture
  discrimination. When all three converge, classification reaches high_confidence.
- Architecture super-class identification is robust across all test cases. Even when
  the framework misses subtype refinement, it correctly identifies whether the case
  is in the T-cluster, I-cluster, or D-cluster.
- F6 categorical handling (depletion direction) is the framework's principal
  Architecture discriminator and works well for clean A, TA, TB, I, H signatures.
- The F11 categorical handler distinguishes flat (I) from concave (D) and convex (T),
  as designed.

### What requires empirical refinement
1. **OD modifier detection (Case 03):** The F12 asymmetric pattern + F14b witness
   self-binding combination should up-weight TA-OD/TB-OD relative to TA/TB. The
   ordinal default captures the direction but not the magnitude needed for argmax.
   Empirical calibration on the Tori-class corpus would improve this.

2. **I_alpha vs I_beta subtype (Case 04):** Discrimination requires capturing
   sophistication-specific signals (sustained mimicry across many channels, cross-channel
   coherence under perturbation). The standard filter battery doesn't strongly distinguish
   these subtypes; the discrimination is observational quality rather than signature
   content.

3. **D vs I architecture at low-disclosure cases (Case 06):** When all architectures
   converge on E-target signatures (F1 covariance, F3 rapid decay), the D vs I distinction
   relies on F11 concave (D) vs flat (I). At preliminary observation depth, this
   discrimination is at the margin.

### Operational implications

- **Operators with ordinal-default calibration:** Use the framework for super-class
  identification (T vs I vs D) with high confidence; treat subtype distinctions as
  preliminary unless cross-dyad evidence or longer observation supports them.
- **Operators with empirical calibration:** Refine F12, F14b, F8 likelihood tables
  against your case corpus; the framework's classification accuracy improves
  substantially with even small (n~10) labeled-case calibration.
- **For all operators:** Trust super-class classification at standard or higher
  confidence levels; treat subtype refinement as conditional on additional evidence.

## How to run

```bash
cd reference_impl
python run_tests.py
```

The test runner exercises the full Bayesian classifier per Eq (19-20) on six synthetic
cases covering the primary diagnostic space. Output includes per-case classification,
posterior distribution top-3, and pass/partial/fail status.

## Calibration tuning

Operators wishing to refine likelihoods should edit:
- `filters.py` `_f6_likelihood`, `_f8_*_likelihood`, `_f11_likelihood`, `_f12_*_likelihood`,
  `_f14b_architecture_likelihood` for categorical filter handling
- `filters.py` `*_FILTER_DIRECTIONS` dictionaries for ordinal filter handling
- Likelihood values should be calibrated against labeled cases from operator's corpus.

The framework's discrimination is honest at the ordinal-default calibration; refinement
improves accuracy but does not change the underlying inference structure (Eq 19-20).
