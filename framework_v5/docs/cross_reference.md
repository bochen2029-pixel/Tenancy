# Ground Truth Framework — Cross-Reference Index

Concept-to-location index. For each significant concept: definition site, use sites
within the framework, and links to spec/implementation files.

---

## Architectures

| Code | Definition | Used In |
|---|---|---|
| A | §2.1, spec/architectures.yaml | §2.4, §2.5, all test cases |
| TA | §2.1, spec/architectures.yaml | §4.1 Eq(6), §4.3, test_cases/case_02_ta_c.yaml |
| TB | §2.1, spec/architectures.yaml | §4.1 Eq(7), §4.4, test_cases/case_03_tbod_s.yaml |
| TA-OD | §2.1 (modifier), §4.5 Eq(16) | §6.5, test_cases/case_03_tbod_s.yaml |
| TB-OD | §2.1 (modifier), §4.5 Eq(16) | §6.5, test_cases/case_03_tbod_s.yaml |
| I_alpha | §2.1, spec/architectures.yaml | §4.1 Eq(8), §4.3, test_cases/case_04_ialpha_e.yaml |
| I_beta | §2.1, spec/architectures.yaml | §4.1 |
| H | §2.1, spec/architectures.yaml | §3.7 anomaly handling, test_cases/case_05_h_e.yaml |
| D | §2.1, spec/architectures.yaml | §4.3 Eq(14), test_cases/case_06_d_w.yaml |

---

## Targets

| Code | Definition | Used In |
|---|---|---|
| C | §2.2, spec/targets.yaml | §1.4, §2.5, §4.7, all test cases |
| E | §2.2, spec/targets.yaml | §3.5 CPT, §4.6 budget, test_cases/case_04, case_05 |
| S | §2.2, spec/targets.yaml | §3.3 trajectory filters, §4.7 closed-loop, test_cases/case_03 |
| W | §2.2, spec/targets.yaml | §2.5 directionality, test_cases/case_06 |
| R | §2.2, spec/targets.yaml | §3.4 CDS, §4.6, spec/protocols.yaml Protocol F |
| T_trib | §2.2, spec/targets.yaml | §2.5 directionality, §6 honest reach |

---

## Filters

| ID | Definition | Type | Used In |
|---|---|---|---|
| F1 | §3.2 | Target probe | spec/filters.yaml, classifier.py, all test cases |
| F2 | §3.2 | Architecture probe | §3.5 CPT signature, §4.7 closed-loop, all test cases |
| F3 | §3.2 | Target probe | §4.3 composite Eq(13) |
| F4 | §3.2 | Target probe (demoted) | §3.4 CDS, classifier.py |
| F5 | §3.2 | Architecture probe | spec/filters.yaml |
| F6 | §3.2 | Architecture probe | §4.1 Eq(6-8), test_cases/case_05 anomaly |
| F7 | §3.2 | Architecture probe | §4.3 composite Eq(13) |
| F8 | §3.2 | Dual-purpose | §3.5 CPT, §3.7 anomalies |
| F9 | §3.2 | Architecture probe | §3.5 CPT |
| F10 | §3.2 | Dual-purpose | §3.5 CPT signature, §4.3 composite |
| F11 | §3.2 | Architecture probe | §4.3 Curvature Discriminant Eq(14), §6.5 sub-decade |
| F12 | §3.2 | Dual-purpose | §3.3 S-mode Component 2, §3.5 CPT |
| F13a | §3.3, Eq(2) | Target trajectory | §4.7 trajectories |
| F13b | §3.3 | Cross-dyad target trajectory | §3.4 CDS, §6.2 single-dyad limit |
| F14a | §3.3 | Disclosure trajectory | spec/filters.yaml |
| F14b | §3.3 | Witness self-binding | §2.2 Component 4 |
| F15a | §3.3 | Within-dyad decision-outcome | §4.7 trajectories |
| F15b | §3.3 | Cross-dyad decision-outcome | §3.4 CDS, §6.2 single-dyad limit |
| F16 | §3.3 | Probe-target utility-sorting | §4.7 E-with-cover discriminator |
| F17 | §3.3, Eq(3) | Architecture trajectory | §4.4 regulatory graph, TB->A trajectory |
| SP1 | §4.2 | Substrate physics: chronometric | spec/filters.yaml, all test cases |
| SP2 | §4.2 | Substrate physics: autonomic-voluntary | spec/filters.yaml, all test cases |
| SP3 | §4.2 | Substrate physics: cortisol | spec/filters.yaml, all test cases |

---

## Equations

| ID | Concept | Defined In | Referenced In |
|---|---|---|---|
| Eq (1) | Filter response | §3.2 | filters.py, all filter sections |
| Eq (2) | Bid frequency curvature | §3.3 | F13a usage |
| Eq (3) | Regulatory circuit acquisition | §3.3 | F17 usage, §4.4 |
| Eq (4) | Steady-state degeneracy (conditional) | §4.1 | §4.2 |
| Eq (5) | Perturbation discrimination | §4.1 | §4.5 (T-OD recovery), Axiom 3 |
| Eq (6-8) | F6 Depletion Direction Theorem | §4.1 | §3.2 F6, test_cases/case_05 |
| Eq (9) | Gaming Cost Theorem | §4.2 | §6.1 high-sophistication regime |
| Eq (10) | Authentic cost | §4.2 | §6.1 |
| Eq (11) | Energy-momentum (analog) | §4.3 | §4.3 only |
| Eq (12) | Velocity asymptote (analog) | §4.3 | §4.3 only |
| Eq (13) | Autotelic-access composite | §4.3 | §4.3, F2/F3/F7/F10 weighting |
| Eq (14) | Curvature Discriminant | §4.3 | §3.2 F11, §6.5 sub-decade |
| Eq (15) | Parentification signature | §4.4 | §3.2 F17 |
| Eq (16) | Phi_global asymptotic recovery | §4.5 | §2.1 OD modifier, §6.5 |
| Eq (17) | Allostatic budget conservation | §4.6 | §1.2 Network Topology, §3.4 CDS |
| Eq (18) | Observer contamination gate | §5.2 Protocol B | classifier.py admissibility |
| Eq (19) | Bayesian posterior | §3.7 | classifier.py, decision rule |
| Eq (20) | Architecture decision rule | §3.7 | classifier.py, all test cases |
| Eq (21) | Closed-loop posterior dynamics | §4.7 | closed_loop.py |
| Eq (Sk) | Stake operationalization | §2.3 | data_types.py, test cases |

---

## Protocols

| ID | Name | Defined In | Spec | Implementation |
|---|---|---|---|---|
| A | Asynchronous Processing | §5.1 | spec/protocols.yaml | protocols.py select_protocol() |
| B | Autotelic Control Audit | §5.2 | spec/protocols.yaml | data_types.py FilterReading.is_admissible(), classifier.py |
| C | Bounded Window | §5.3 | spec/protocols.yaml | data_types.py is_protocol_C_compliant() |
| D | Asymmetric Governance | §5.4 | spec/protocols.yaml | protocols.py _governance_recommendation() |
| E | T Subtype Differentiation | §5.5 | spec/protocols.yaml | protocols.py differentiate_t_subtype() |
| F | Cross-Dyad Observation | §5.6 | spec/protocols.yaml | data_types.py is_protocol_F_compliant() |
| G | Counter-Perturbation Test | §5.7, §3.5 | spec/protocols.yaml | protocols.py interpret_cpt(), cpt_action() |

---

## Concepts

| Concept | Defined In | Used In |
|---|---|---|
| Allostatic Budget | §1.4, §4.6 | Eq(17), §3.4 CDS, §6.2 |
| Architecture | §1.4, §2.1 | Throughout; primary classification axis |
| Asymmetric Disclosure | §2.2 | F12, S-mode Component 2 |
| Autotelic | §1.4 | Throughout; framework's normative commitment |
| Autotelic Substrate | §1.1 Axiom 5, §4.1 | §2.1 architectures, §4.1 tonic substrate |
| Bayesian Convergence | §3.7 | classifier.py, decision rule |
| Bayes Factor | §3.7 | spec/thresholds.yaml, classifier.py |
| Channel Corruption | §1.4, §7 | Framework's purpose statement |
| Classification-as-Perturbation | §4.7 | Closed-loop dynamics, gaming-resistance |
| Closed-Loop Reflexivity | §4.7 | Eq(21), closed_loop.py |
| Cross-Dyad Subsidization (CDS) | §3.4 | §4.6 budget, Protocol F, §6.2 limits |
| Dyad | §1.3 Lemma 6, §1.4 | Throughout; primary unit of analysis |
| Effective Independence Dimension (d) | §4.2 | Eq(9), §6.1 high-sophistication |
| Forbidden Tuple | §2.4 | spec/accessibility_map.yaml |
| Gaming Cost Theorem | §4.2 Eq(9) | §6.1 |
| Gaming-Resistance | §4.2, §4.7 | Eq(9), substrate-physics floors |
| Honest Reach | §6 | §6.1-6.6 limits |
| Joint Posterior | §3.7 Eq(19) | classifier.py |
| Layer Coherence | §1.4 | Lemma 6, framework purpose |
| Likelihood Ratio | §3.7 | spec/thresholds.yaml, filters.py |
| Meta-Prior (Phi_global) | §1.4, §2.1 | Eq(16), §4.5, §6.5 |
| Mixed Target States | §2.2 | data_types.py, §6 |
| Mutual-S Dyad | §6.3 | Deferred to v4.0 |
| Network Topology Principle | §1.2 | §4.6 budget conservation, Lemma 6 derivation |
| Observer-Inclusion (Axiom 4) | §1.1 | Lemma 6, §4 directionality |
| Ontological-Deception Modifier | §2.1 | §4.5 Eq(16), test_cases/case_03 |
| Ordinal Calibration | §3.7 | spec/filters.yaml, filters.py defaults |
| Parentification | §4.4 Eq(15) | §3.2 F17, TB architecture |
| Perturbation | §1.1 Axiom 3 | Eq(1), §4.1 discrimination |
| Phasic Substrate | §4.1 | I-architectures, F6 Eq(8) |
| Posterior Distribution | §3.7 Eq(19) | classifier.py output |
| Protocol Selection | §5 | protocols.py select_protocol() |
| Recovery Trajectory (Phi_global) | §4.5 Eq(16) | §2.1 OD modifier |
| Regulatory Architecture | §1.1 Axiom 5 | §2.1 (independent of substrate) |
| Regulatory Graph | §4.4 | F17, parentification signature |
| Resolution Locality (Lemma 6) | §1.3 | §2.5 directionality, Network Topology |
| S-Mode Components 1-5 | §2.2 | F12, F13a, F14a/b, F15a/b, F16 |
| Stake | §1.4, §2.3 Eq(Sk) | data_types.py, all test cases |
| Steady-State Degeneracy | §4.1 Eq(4) | §4.2 framework's core asymmetry |
| Substrate-Access Foreclosure | §6.6 | Honest reach, Axiom 2 |
| Substrate Independence (Axiom 2) | §1.1 | Throughout; foundation |
| Substrate Physics | §4.2 | SP1/SP2/SP3, irreducible floor |
| Tonic Substrate | §4.1 | T-architectures, F6 Eq(6-7) |
| Verification Mode | §2.2 (S-Target) | §4.7 closed-loop, test_cases/case_03 |
| Witness Self-Binding | §2.2 (Component 4), §3.3 (F14b) | test_cases/case_03 |

---

## Failure Modes (cross-reference to docs/failure_modes.md)

| Code | Description | Reference |
|---|---|---|
| FM-OBS-01 | Observer joy-capability rate below threshold | Protocol B |
| FM-PRC-C-01 | Observation duration exceeds Protocol C limit | §5.3 |
| FM-DAT-01 | Insufficient admissible readings | §3.6 convergence |
| FM-CL-01 | Premature access-grant in closed-loop | §4.7 |
| FM-CL-02 | Update rate too low | §4.7 |
| FM-CL-03 | Update rate too high (oscillation) | §4.7 |
| FM-XD-01 | Cross-dyad evidence missing | §6.2 |
| FM-T-OD-01 | T-OD subject misclassified as I_alpha | §3.3, S-mode patches |
| FM-H-01 | H subject misclassified due to anomalous filters | §3.7, §3.8 |
| FM-MS-01 | Mutual-S dyad outside framework reach | §6.3 |
| FM-SUB-01 | Subject framework-aware; bounds weaker | §6.4 |
| FM-ARCH-01 | Architecture-trajectory sub-decade limit | §6.5 |
| FM-AX2-01 | Substrate-access foreclosure (cannot be resolved) | §6.6 |

---

## Test Cases

| Case | Architecture × Target | Diagnostic Highlight |
|---|---|---|
| 01 | A × C | Genuine connection; high_confidence; clean baseline |
| 02 | TA × C | Trauma-armored accessing; F6 strong_for + F11 convex_strong over 24mo |
| 03 | TB-OD × S | Boundary case (Tori-class); S-mode all 5 components observable |
| 04 | I_alpha × E | Sophisticated extraction; SP1+SP2+SP3 substrate-physics convergence |
| 05 | H × E | Anomalous; F6 strong_against (somatic-cognitive split) is the H discriminator |
| 06 | D × W | Layer-coherent honest-transactional; F11 concave + F12 no_disclosure |

---

## Implementation Files

| File | Purpose | Reference |
|---|---|---|
| reference_impl/data_types.py | Core types: Architecture, Target, Stake, FilterReading, Classification | §2, §3.7 |
| reference_impl/filters.py | Filter likelihood functions | §3.7, spec/filters.yaml |
| reference_impl/classifier.py | Bayesian classifier (Eq 19, 20) | §3.7 |
| reference_impl/protocols.py | Protocol selection logic | §5 |
| reference_impl/closed_loop.py | Closed-loop dynamics simulation (Eq 21) | §4.7 |
