# Ground Truth Framework — Failure Mode Catalog

Catalogued failure modes with operational responses. When the framework's classification
is unreliable, it is usually because one of these modes is active. Detection and
response procedures specified per mode.

---

## Observer-Side Failure Modes

### FM-OBS-01: Joy-Capability Rate Below Threshold
**Detection:** `observer_joy_capability_rate < 0.3` per Protocol B.
**Mechanism:** Observer was too analytically occupied during interactions to be genuinely present. Filter readings are systematically biased toward extraction-detection because contaminated observations under-report substrate signatures.
**Operational Response:**
1. Stop applying the matrix.
2. Re-establish observer's autotelic capacity outside the framework (other relationships, autotelic activities, rest).
3. Resume framework application only when joy-capability rate is sustained above 0.3 across multiple interactions.
4. Re-classify with the contaminated observations excluded.

**Reference:** §5.2 Protocol B, Eq (18)

---

### FM-OBS-02: Observer Phi_global Contaminated
**Detection:** Observer self-audit reveals that observer is themselves operating under T-OD modifier OR has accumulated channel corruption sufficient to suppress baseline trust.
**Mechanism:** Observer's contaminated meta-prior biases all classifications toward false-E. Subjects with substrate signatures get misclassified because the observer's prior weight on "this is genuine" is artificially low.
**Operational Response:**
1. Acknowledge the contamination explicitly in classification notes.
2. Apply Bayesian posterior with elevated rho_threshold (require rho > 30 instead of rho > 10) to compensate.
3. Consider whether observer's own framework-application is functioning as armor against connection rather than as diagnostic tool.
4. Decommission matrix temporarily; engage in channel-rehabilitation interactions.

**Reference:** §5.8 Observer's Own Architecture

---

### FM-OBS-03: Mid-Interaction Analysis (Protocol A Violation)
**Detection:** Observer notices analytical thoughts about classification during interaction itself rather than only during asynchronous reflection.
**Mechanism:** Mid-interaction analysis contaminates the data. Subject's responses are no longer to genuine engagement but to observer's guarded/analytical state.
**Operational Response:**
1. Recognize the violation.
2. Allow subject's response from that interaction to be marked as contaminated (do not admit to evidence).
3. Re-establish presence in subsequent interactions.
4. Track whether Protocol A violations are increasing; that itself is a signal that the matrix is operating as armor.

**Reference:** §5.1 Protocol A

---

## Data/Evidence Failure Modes

### FM-DAT-01: Insufficient Admissible Readings
**Detection:** Number of admissible readings (post-Protocol B filtering) below 5.
**Mechanism:** Classification is statistically underdetermined. Bayesian posterior is dominated by prior; filters do not yet convey enough information.
**Operational Response:**
1. Extend observation window.
2. Identify which filter axes lack evidence.
3. Watch for the perturbation events that would activate them.
4. Do NOT ship a classification at this state; output "insufficient data" per Protocol B.

**Reference:** §3.6 convergence thresholds

---

### FM-DAT-02: Filters Pointing in Different Directions
**Detection:** Posterior over Architecture has no dominant cell; `bayes_factor < 3` (below preliminary threshold).
**Mechanism:** Multiple architectures fit the evidence approximately equally. Could indicate (a) Case H (somatic-cognitive decoupling), (b) Architecture-Target mismatch outside accessibility map, (c) cross-dyad subsidization producing anomalous within-dyad signatures, (d) filter operating outside valid conditions.
**Operational Response:**
1. Examine which filters are pointing where.
2. If F6 anomaly + SP2 both_present + agenda-dominant cognitive markers: consider H architecture.
3. If F11 convex but F1 covariance > 0: consider Architecture trajectory (recovery in progress).
4. Do NOT force a categorical classification; report posterior distribution and recommend extending observation.

**Reference:** §3.7 Bayesian formulation, §3.8 anomalies

---

### FM-DAT-03: Filter Reading Out of Valid Range
**Detection:** Filter reading produced under conditions where the filter does not validly apply (e.g., F11 within first 6 months; F4 without cross-dyad observation).
**Mechanism:** Out-of-range readings produce uninformative likelihoods that should not contribute to the posterior.
**Operational Response:**
1. Mark the reading as out-of-range.
2. Exclude from posterior computation.
3. Re-classify without it.

**Reference:** spec/filters.yaml validity conditions

---

## Closed-Loop Dynamics Failure Modes

### FM-CL-01: Premature Access-Grant
**Detection:** Observer's access-grant function `g(q)` produces high access at posterior `q < 0.7`.
**Mechanism:** E-mode-with-cover subjects extract before the closed-loop bracket has converged. Per §4.7, do not grant high-Stake access at posterior probability below 0.7.
**Operational Response:**
1. Tighten access-grant function: `g(q) ≤ 0.5` for `q < 0.7`.
2. Restart closed-loop with corrected access-grant calibration.
3. Acknowledge observer-side judgment error; do not blame subject for taking advantage of correctly-detected opportunity.

**Reference:** §4.7 closed-loop failure modes, spec/thresholds.yaml

---

### FM-CL-02: Update Rate Too Low
**Detection:** Posterior `q` not converging within expected number of observations; `eta` too small.
**Mechanism:** Cross-dyad subsidization can fund continued cover-performance during slow-convergence window. The subject's surface presentation remains S-coherent because budget for cover is being subsidized from extractive dyads elsewhere.
**Operational Response:**
1. Increase `eta` (more aggressive Bayesian updating per observation).
2. Apply Protocol F (cross-dyad observation) to detect CDS directly.
3. If CDS confirmed, classify via Network Topology Model rather than within-dyad alone.

**Reference:** §4.7, §3.4 CDS, §4.6

---

### FM-CL-03: Update Rate Too High (Oscillation)
**Detection:** `|q(t+dt) - q(t)| > 0.3` sustained over 3+ updates per `is_oscillating()`.
**Mechanism:** Single-event noise dominates the update; posterior bounces between fixed points without converging.
**Operational Response:**
1. Reduce `eta` by factor of 2.
2. Smooth observation window: aggregate filter readings across multiple events before each update.
3. Continue observation; oscillation usually resolves once enough independent perturbation events accumulate.

**Reference:** §4.7

---

## Cross-Dyad / Network Topology Failure Modes

### FM-XD-01: Cross-Dyad Evidence Missing
**Detection:** Observer has access to only 1 dyad; Protocol F requirement of 3+ dyads not satisfied.
**Mechanism:** Single-dyad observers cannot detect cross-dyad subsidization, R-mode beyond single-dyad, or sophisticated E-mode-with-cover that uses cross-dyad selectivity.
**Operational Response:**
1. Explicitly bound classification by acknowledging single-dyad epistemic limit.
2. Use higher rho_threshold (> 30 instead of > 10) for shipping classification.
3. Accept that classification is bounded; some failure modes will not be detectable.

**Reference:** §6.2 single-dyad observer epistemic limits

---

### FM-XD-02: Cross-Dyad Subsidization Suspected But Not Confirmed
**Detection:** Within-dyad signals are anomalously consistent with high-cost C/S maintenance, but observer cannot verify whether other dyads show E-pattern.
**Mechanism:** Subject may be maintaining performance-Target in this dyad funded by extraction in unobserved dyads. Without cross-dyad data, the subject appears to be substrate-rich; with cross-dyad data, the subsidization pattern would emerge.
**Operational Response:**
1. Treat the within-dyad classification as upper-bounded by suspected subsidization.
2. If access to additional dyads becomes available, expand observation to confirm.
3. If not, ship classification with explicit "cross-dyad subsidization not ruled out" annotation.

**Reference:** §3.4 CDS, §4.6

---

## Architecture-Specific Failure Modes

### FM-T-OD-01: T-OD Subject Misclassified as I_alpha
**Detection:** Subject classified I_alpha/E but exhibits S-mode Components 1-5 and substrate-physics signatures incompatible with I.
**Mechanism:** The original framework collapsed S-Target with E-Target. T-OD subjects in S-mode produce surface signatures (probing, asymmetric disclosure, decision-deferral) that look like sophisticated extraction but are actually verification per Eq (16) recovery requirements.
**Operational Response:**
1. Apply patched framework's S-Target derivation.
2. Check Components 1-5 of S-mode signature; especially Component 4 (witness self-binding with corruption check).
3. Check substrate-physics: SP1 sub-100ms, SP2 both_present, SP3 hour-scale persistence indicate T not I.
4. Re-classify as TA-OD or TB-OD with S-Target.

**Reference:** §2.2 S-Target derivation, test_cases/case_03_tbod_s.yaml

---

### FM-H-01: H Subject Misclassified Due to Anomalous Filters
**Detection:** F6 anomalous (somatic-cognitive split); SP2 both_present despite cognitive-extraction markers; intermediate SP1 chronometric lag.
**Mechanism:** H architecture has T-substrate that has been prefrontally endorsed for extraction. Somatic markers persist (autonomic, cortisol) but cognitive operation is I. Standard filter battery produces inconsistent signals.
**Operational Response:**
1. Recognize the anomaly pattern: substrate signature + extraction operation = H.
2. Apply F6 strong_against reading specifically (somatic-cognitive split signature).
3. Use H-specific likelihood pattern from spec/filters.yaml.
4. Classification produces standard confidence (rho > 10) but rarely high_confidence (rho > 30) due to anomaly handling.

**Reference:** §3.7, §3.8 anomalies, test_cases/case_05_h_e.yaml

---

### FM-MS-01: Mutual-S Dyad Outside Framework Reach
**Detection:** Both observer and subject exhibit S-mode signatures toward each other; verification protocols become entangled.
**Mechanism:** Each party's verification-bid event is the other's verification-bid event. Coordination problem: someone must demonstrate C-disposition first, but both wait for evidence first. Schelling problem requiring coordination-theoretic layer.
**Operational Response:**
1. Acknowledge that mutual-S is outside the current framework's reach (deferred to v4.0).
2. Frameworks deferred extension requires coordination-theoretic layer.
3. Provisional response: explicit communication about the mutual-S configuration may itself break the symmetry, but this requires both parties to be framework-aware.

**Reference:** §6.3 Mutual-S deferred

---

### FM-SUB-01: Subject Framework-Aware (Weaker Gaming Bounds)
**Detection:** Subject demonstrates explicit knowledge of the framework or similar diagnostic systems; behavior reflects meta-cognitive optimization over substrate-physics constraints.
**Mechanism:** Framework-aware subjects can calibrate consciously within substrate-physics floors. Cortisol kinetics, chronometric lag, autonomic-voluntary split cannot be overridden, but within those constraints conscious calibration is possible. Gaming-resistance bounds are weaker against framework-aware subjects.
**Operational Response:**
1. Increase weight on substrate-physics filters (SP1, SP2, SP3) relative to behavioral filters.
2. Use higher rho_threshold (> 30) for classification.
3. Acknowledge in classification notes that subject framework-awareness is suspected and bounds are weaker.

**Reference:** §6.4

---

## Architecture-Trajectory Failure Modes

### FM-ARCH-01: Architecture-Trajectory Sub-Decade Limit
**Detection:** Observer wants to assess Architecture-shift but observation window is < 36 months.
**Mechanism:** `tau_A / tau_T ~ 10^2 to 10^3` means standard observation windows cannot definitively measure Architecture-shift.
**Operational Response:**
1. Use cross-dyad breadth observation as partial substitute (Architecture-evidence integrates across simultaneous dyads).
2. Cross-dyad reduces effective window from 12-36 months to 6-12 months.
3. For sub-6-month Architecture questions: accept that they remain unanswerable within standard windows.

**Reference:** §6.5

---

## Axiomatic Failure Modes

### FM-AX2-01: Substrate-Access Foreclosure
**Detection:** Operator wants certainty about subject's "actual inner experience" matching their behavior.
**Mechanism:** The framework's classification operates over operational-equivalence classes per Axiom 2. It cannot answer "does this person have an inner experience that matches their behavior" - that is outside Axiom 2's scope and outside the framework's reach by axiomatic commitment.
**Operational Response:**
1. Recognize that this is structural foreclosure, not framework defect.
2. The framework reduces uncertainty; it does not eliminate it.
3. The terminal commitment to invest is the operator's responsibility, not the framework's output.
4. Use the framework's bounded classifications as inputs to your decision; the residual uncertainty is your commitment.

**Reference:** §6.6

---

### FM-AX3-01: Observer-Side Probing Engineered (Axiom 3 Violation)
**Detection:** Observer engineers perturbation events specifically to test the subject (other than Protocol G CPT).
**Mechanism:** Observer-side probing contaminates exactly the signals the framework is designed to measure. The subject's response is to the test, not to genuine perturbation.
**Operational Response:**
1. Recognize the violation.
2. Exclude the engineered observation from evidence.
3. Wait for natural perturbation events (life supplies them per Axiom 3).
4. CPT is the only sanctioned exception, and only as last-resort discriminator with explicit cost.

**Reference:** §1.1 Axiom 3, §3.5 CPT exception

---

## High-Sophistication Regime

### FM-HS-01: High-Sophistication Regime Collapse
**Detection:** Subject's gaming dimension `d >= 5-6` sustained over decade timescales; gaming cost converges to authentic cost per Eq (9).
**Mechanism:** At sufficient sophistication, the cost of gaming converges to the cost of being authentic. In this regime, S-mode and sophisticated E-with-cover are observer-indistinguishable AND under Axiom 2 they are decision-theoretically equivalent.
**Operational Response:**
1. The framework gives up classification at this regime by design.
2. Treat the subject as authentic for all decision-theoretic purposes (Axiom 2).
3. This is not a defect; it is the framework's honest reach.
4. The very-high-sophistication regime is rare; most subjects operate at `d ≤ 3` where the framework discriminates.

**Reference:** §6.1 high-sophistication regime collapse

---

## Summary Table

| Code | Mode | Detection | Response |
|---|---|---|---|
| FM-OBS-01 | Joy-capability low | Rate < 0.3 | Stop matrix; rehabilitate |
| FM-OBS-02 | Observer Phi_global contaminated | Self-audit | Elevated rho_threshold |
| FM-OBS-03 | Protocol A violation | Mid-interaction analysis | Exclude observation |
| FM-DAT-01 | Insufficient readings | < 5 admissible | Extend observation |
| FM-DAT-02 | Filters disagree | Bayes factor < 3 | Report posterior, extend |
| FM-DAT-03 | Reading out of range | Validity conditions failed | Exclude from posterior |
| FM-CL-01 | Premature access | g(q) > 0.5 at q < 0.7 | Tighten g(q) |
| FM-CL-02 | Update too low | Slow convergence | Increase eta; apply Protocol F |
| FM-CL-03 | Update too high | Oscillation | Reduce eta; smooth window |
| FM-XD-01 | Cross-dyad missing | Single dyad only | Bound classification |
| FM-XD-02 | CDS suspected | Within-dyad anomaly | Expand observation if possible |
| FM-T-OD-01 | T-OD as I_alpha | Components 1-5 + substrate physics | Apply S-Target patch |
| FM-H-01 | H anomaly | Somatic-cognitive split | H-specific likelihood |
| FM-MS-01 | Mutual-S | Both parties verifying | Outside reach (v4.0) |
| FM-SUB-01 | Framework-aware | Subject knows framework | Higher rho_threshold |
| FM-ARCH-01 | Sub-decade trajectory | Window < 36mo | Cross-dyad substitute |
| FM-AX2-01 | Substrate foreclosure | Want inner certainty | Accept structural limit |
| FM-AX3-01 | Engineered probing | Observer creates events | Exclude; wait for natural |
| FM-HS-01 | High-sophistication | d >= 5 sustained | Treat as authentic per Axiom 2 |
