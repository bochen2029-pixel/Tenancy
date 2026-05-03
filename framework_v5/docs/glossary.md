# Ground Truth Framework — Glossary

Concept definitions for the framework. Each entry includes definition, formal notation
where applicable, and reference locations.

---

## A

**A (Architecture: Autotelic-Coherent).** The reference architecture. Substrate intact, regulatory architecture intact, meta-prior `Phi_global` at baseline. Accessible targets: `{C, W, S}`.
*Reference: §2.1, spec/architectures.yaml*

**Accessibility Map.** Specification of which `(Architecture, Target)` tuples are structurally accessible vs forbidden vs conditional. Without the map, the three-axis classification permits incoherent classifications.
*Reference: §2.4, spec/accessibility_map.yaml*

**Allostatic Budget.** The unified metabolic envelope a subject operates within. Cross-dyad subsidization (per Network Topology Principle) is a budget-balancing strategy.
*Reference: §1.4, §4.6, Eq (17)*

**Architecture (A).** The subject's long-term substrate weighting across relational optimization processes. Property of the subject. Time-constant: years to decades.
*Reference: §1.4, §2.1*

**Asymmetric Disclosure.** Component 2 of S-mode signature. Surface vulnerability is high (recruitment-shaped offerings); deep vulnerability is low (load-bearing material remains protected).
*Reference: §2.2*

**Asymptotic Recovery (Phi_global).** The recovery trajectory for T-OD subjects' meta-prior, modeled by Eq (16): `Phi_global(N, T) = Phi_baseline - (Phi_baseline - Phi_initial) * exp(-(N/N_0) - (T/T_0))`. Asymptotic toward population baseline as counterexamples and time accumulate.
*Reference: §4.5*

**Autotelic.** Valued for its own sake. Greek *auto* (self) + *telos* (end). An autotelic interaction is its own purpose. The presence is the content.
*Reference: §1.4*

**Autotelic Substrate.** Neural architecture built during the attachment window (0-24 months) through repeated cycles of infant distress -> caregiver attunement -> co-regulation -> return to homeostasis. The substrate that makes social presence intrinsically rewarding.
*Reference: §1.1 Axiom 5*

**Autotelic Termination Requirement (Axiom 1).** Every instrumental chain must terminate in something valued for its own sake, or the chain is structurally ungrounded.
*Reference: §1.1*

**Axioms.** The framework's five load-bearing axiomatic commitments plus the Network Topology Principle. (Axiom 1: Autotelic Termination; Axiom 2: Substrate Independence; Axiom 3: Perturbation Sufficiency; Axiom 4: Observer-Inclusion; Axiom 5: Regulatory Architecture Independence; plus Network Topology Principle.)
*Reference: §1*

---

## B

**Bayes Factor.** Ratio of posterior probabilities for the most-likely classification vs second-most-likely. Used in the decision rule per Eq (20). Thresholds: `rho = 3` preliminary, `rho = 10` standard, `rho = 30` high-confidence.
*Reference: §3.7, spec/thresholds.yaml*

**Bayesian Convergence Formulation.** The framework's underlying inferential structure. Filter readings update beliefs about (Architecture, Target, Stake) tuple via Eq (19). Replaces the heuristic "at least 3 filters" rule.
*Reference: §3.7*

**Bid Frequency Curvature.** Filter F13a measurement: `kappa_bid = d/dt [bid_frequency_in_dyad(t)]`. Discriminates S-mode trajectory: `kappa < 0` for S->C, `kappa ~ 0` for persistent S, `kappa > 0` for S->E.
*Reference: §3.3, Eq (2)*

**Boundary-Diffuse (TB).** Architecture with intact substrate but disrupted regulatory architecture, typically through parentification. Out-degree dominant regulatory graph; rho > 5 per Eq (15).
*Reference: §2.1*

**Bounded Window (Protocol C).** Protocol limiting observation duration to prevent diagnostic from converting into relational pathology. Initial classification: 9 months max; F11 longitudinal extension: 36 months max; post-classification: matrix permanently OFF.
*Reference: §5.3*

---

## C

**C (Target: Committed-Autotelic).** Counterpart valued as terminal good. Observer is included in the autotelic terminus. The relationship's value does not depend on what it produces.
*Reference: §2.2*

**Calibration Source.** Annotation on each filter likelihood entry indicating whether the value is `substrate_physics` (derived from biology), `ordinal_default` (5-point scale), or `case_corpus_estimate` (from labeled cases).
*Reference: spec/filters.yaml*

**Case I.** Subject classified as I-architecture (I_alpha, I_beta, or H). Operative target is E. Surface presentation may mimic autotelic via PFC simulation but substrate-physics filters discriminate.
*Reference: §2.1, §4.1*

**Case T.** Subject classified as T-architecture (TA, TB, TA-OD, TB-OD). Substrate intact; armor or regulatory disruption modulates surface presentation. Convex F11 returns under safe investment.
*Reference: §2.1, §4.3*

**Channel Corruption.** Degradation of the observer's capacity to trust autotelic signals, caused by accumulated exposure to undetected layer incoherence. The framework is channel-rehabilitation infrastructure.
*Reference: §1.4, §7*

**Chronometric Lag.** Substrate-physics filter SP1: genuine subcortical affect onset 50-100ms; PFC-routed simulation onset 250-400ms. The latency gap is the discriminator.
*Reference: §4.2*

**Classification-as-Perturbation Property.** The framework's classification of subject as S->C trajectory is itself a perturbation event that completes the verification. Observer's response to classification feeds back into next classification update; closed-loop dynamics per §4.7.
*Reference: §4.7, Eq (21)*

**Closed-Loop Reflexivity.** The framework operates as a closed-loop dynamical system, not an open-loop classifier. Stability analysis requires examining fixed points (q*=1 committed, q*=0 rejected) and failure modes (premature access-grant, update-rate misconfiguration).
*Reference: §4.7*

**Conditional Independence Assumption.** Filter readings are assumed conditionally independent given (A, T, Sk). Holds approximately when filters probe distinct perturbation axes. For correlated filters (e.g., F1 and F4), use joint likelihood directly.
*Reference: §3.7*

**Conditional Tuple.** An (Architecture, Target) pair that is accessible only under specific conditions (e.g., TA-OD x C achievable only post Phi_global recovery).
*Reference: §2.4, spec/accessibility_map.yaml*

**Counter-Perturbation Test (CPT, Protocol G).** Last-resort discriminator for high-stake S->C/E ambiguity. Observer artificially and irrevocably removes the possibility of the deferred transaction. Violates Axiom 3; consumes observer credibility.
*Reference: §3.5, §5.7*

**Cross-Dyad Observation (Protocol F).** Required for cases suspected of cross-dyad subsidization, R-mode, or sophisticated E-mode-with-cover. Observation breadth of at least 3 dyads with sufficient access depth.
*Reference: §5.6, spec/protocols.yaml*

**Cross-Dyad Subsidization (CDS).** Subsidization signature where total extraction value across E-dyads correlates temporally with high-cost performance maintenance in C-or-S dyads. Detectable only with cross-dyad observation breadth.
*Reference: §3.4, §4.6*

**Curvature Discriminant.** Eq (14): `kappa = d^2(A) / d(tau)^2` discriminates Case T (kappa > 0) from Case D (kappa < 0). Third case `|kappa| < kappa_noise` indicates insufficient signal or A near access ceiling.
*Reference: §4.3*

---

## D

**D (Architecture: Developmental Absence).** The substrate that A architecture exhibits was never built. F11 concave; no convex returns under safe investment.
*Reference: §2.1*

**Decision Rule (Eq 20).** `A_hat = argmax P(A | data); ship A_hat iff P(A_hat | data) / P(A_2nd | data) > rho_threshold`.
*Reference: §3.7*

**Depletion Direction Theorem (F6).** Eq (6-8): under cognitive depletion, TA instrumentalism intensifies, TB parentified posture drops, I instrumentalism degrades. The framework's principal architecture discriminator.
*Reference: §4.1*

**Disclosure Hierarchy (F12).** Filter measuring depth and organization of disclosure. Genuine progresses through regulatory-graph stages; strategic disclosure plateaus at recruitment-depth.
*Reference: §3.2*

**Dual-Purpose Filter.** Filter that signals both Architecture and Target (F8, F12). Shape signals one axis; scale signals the other.
*Reference: §3.1*

**Dyad.** A two-party relationship. The framework's primary unit of analysis. Per Lemma 6, classification is dyadic, not subject-level.
*Reference: §1.3*

---

## E

**E (Target: Extractive).** Counterpart valued as resource. Observer excluded from terminal node set. Engagement covaries with subject's instrumental return.
*Reference: §2.2*

**Effective Independence Dimension (d).** Number of mutually-orthogonal perturbation axes the framework probes. Standard regime `d ~ 6-8`; gaming-resistance per Eq (9) requires `d >= 5`.
*Reference: §4.2*

**Ego-Syntonic Hybrid (H).** Architecture with trauma-origin substrate that has been prefrontally endorsed and integrated into coherent extraction-policy. Functionally I_alpha but with somatic-cognitive decoupling producing anomalous filter signals.
*Reference: §2.1*

**Engagement-Utility Covariance (F1).** Filter measuring whether subject's engagement tracks observer's instrumental utility. Primary E-Target identifier.
*Reference: §3.2*

**Epistemic Exhaust (F7).** Filter measuring memory frame: what details are preserved (atmospheric vs agenda-dominant). Substrate signature.
*Reference: §3.2*

**Equation Index.** Reference list of all 21 framework equations plus Eq(Sk).
*Reference: framework.md Appendix B, spec/equations.yaml*

**Extraction-Policy.** A coherent strategy for resource extraction from relationships, prefrontally endorsed (in I_alpha and H) or banal (in I_beta).
*Reference: §2.1*

---

## F

**Filter (Generic).** A measurement device for behavioral output along a specific perturbation axis. Eq (1): `sigma_k(t) = partial(B(t)) / partial(P_k(t))`.
*Reference: §3.2*

**Filter Battery.** The framework's 17 standard filters (F1-F12 original + F13a/b, F14a/b, F15a/b, F16, F17 new) plus 3 substrate-physics filters (SP1, SP2, SP3).
*Reference: spec/filters.yaml*

**Friction Tax (F10).** Filter measuring affective response to inefficiency in shared time. Tolerance signals Architecture; current friction signals Target.
*Reference: §3.2*

**Forbidden Tuple.** An (Architecture, Target) pair that is structurally impossible (e.g., A x E, D x C). Zero-prior cells in the accessibility map.
*Reference: §2.4, spec/accessibility_map.yaml*

---

## G

**Gaming Cost Theorem.** Eq (9): `C_total >= O(2^d)` as `d >> 1`. Lower bound on metabolic cost of simulating across d independent perturbation axes. Authentic cost `C_auth = O(1)`.
*Reference: §4.2*

**Gaming-Resistance.** The framework's structural property of being expensive to fake at sufficient `d`. Gaming-resistance derives from Eq (9) plus substrate-physics floors (chronometric, autonomic, cortisol).
*Reference: §4.2, §4.7*

---

## H

**H (Architecture: Ego-Syntonic Hybrid).** See Ego-Syntonic Hybrid.
*Reference: §2.1*

**High-Sophistication Regime.** The regime where `d >= 5-6` sustained gaming converges to authentic cost; framework collapses S/E under Axiom 2. Honest reach limit.
*Reference: §6.1*

**Honest Reach.** The framework's six structural limits, named honestly rather than hidden: high-sophistication regime collapse, single-dyad observer epistemic limits, mutual-S deferred, subject framework-awareness, sub-decade architecture-trajectory, substrate-access foreclosure.
*Reference: §6*

---

## I

**I_alpha (Architecture: Strategic Instrumental).** Phasic substrate; PFC-driven conscious extraction. Surface presentation can mimic any target via PFC simulation but operative is E.
*Reference: §2.1*

**I_beta (Architecture: Banal Instrumental).** Phasic extraction without PFC sophistication for sustained mimicry. Openly transactional or naively transactional.
*Reference: §2.1*

**Instrumental.** Valued for what it leads to. An instrumental relationship is structured around means-end chains.
*Reference: §1.4*

**Investment Return Curvature (F11).** Filter measuring long-term substrate transformation under safe investment. Convex = Case T; concave = Case D. Principal long-run discriminator.
*Reference: §3.2, §4.3*

---

## L

**Layer Coherence.** The condition in which the relational layer a person presents matches the relational layer they are actually operating on within a specific dyad. The framework's diagnostic concern is layer incoherence.
*Reference: §1.4*

**Lemma 6 (Resolution Locality).** The optimization-target of a relationship is local-to-the-relationship rather than global-to-the-subject. Derived from Axiom 4 + Network Topology Principle.
*Reference: §1.3*

**Likelihood Ratio.** `P(reading | A_1) / P(reading | A_2)`. Used in Bayes-factor decision rule. Default ordinal ratios: 5:1, 2:1, 1:1, 1:2, 1:5 for strong-for, weak-for, neutral, weak-against, strong-against.
*Reference: §3.7, spec/thresholds.yaml*

**Longitudinal Window.** Observation duration sufficient for F11 curvature measurement. Standard 12-36 months for Case T classifications. Sub-decade limit per §6.5.
*Reference: §5.3, §6.5*

---

## M

**Meta-Prior (Phi_global).** The subject's prior probability that any given counterpart's presented relational layer corresponds to operative relational layer. Suppressed by ontological deception (T-OD modifier); recovers per Eq (16).
*Reference: §1.4, §2.1, §4.5*

**Mixed States (Targets).** Targets are formally probability distributions over `{C, E, S, W, R, T_trib}`; categorical classification ships argmax. Mixed states (C+R, S+E) are real and acknowledged.
*Reference: §2.2*

**Mutual-S Dyad.** Configuration where both parties operate S-Target with each other. Verification protocols become entangled; coordination-theoretic extension required (deferred to v4.0).
*Reference: §6.3*

---

## N

**Network Topology Principle.** Subject operates on unified allostatic budget; Targets are locally expressed but globally budgeted. Cross-dyad subsidization is structurally available.
*Reference: §1.2, §4.6*

---

## O

**Observer-Inclusion Criterion (Axiom 4).** The autotelic terminus must include the observer as participant in terminal value, not as resource consumed.
*Reference: §1.1*

**Ontological-Deception Modifier (-OD).** Suffix modifier applied to TA or TB. Catastrophically suppresses Phi_global; recovery per Eq (16) with N_0 large, T_0 in years for severe cases.
*Reference: §2.1*

**Ordinal Calibration.** Default likelihood specification using 5-point scale (strong_for, weak_for, neutral, weak_against, strong_against) with likelihood ratios 5:1, 2:1, 1:1, 1:2, 1:5. Adequate for framework discrimination regime; refines to empirical calibration with case corpus.
*Reference: §3.7*

---

## P

**Parentification Signature.** Eq (15): `rho(s) = out_degree(s) / (in_degree(s) + epsilon)`. Thresholds: `rho > 5` flag, `rho > 10` strong, `rho > 20` severe. Population baseline 0.5-3.
*Reference: §4.4*

**Perturbation.** A change in conditions that life supplies (utility fluctuation, friction, fatigue, silence, loss). The framework observes responses to natural perturbations; observer-side perturbation engineering is forbidden by Axiom 3.
*Reference: §1.1 Axiom 3*

**Phi_global.** See Meta-Prior.

**Posterior.** Eq (19): `P(A, T, Sk | F_1, ..., F_K) ∝ Π_k P(F_k | A, T, Sk) * P(A, T, Sk)`. The framework's central inference output.
*Reference: §3.7*

**Protocol.** An operational rule governing how the framework is applied. Seven protocols A-G.
*Reference: §5, spec/protocols.yaml*

---

## R

**R (Target: Regulatory-Subcontracting).** Counterpart utilized as external biological exoskeleton. Bond is substrate-dependent on regulatory function, not the person. Most commonly missed mode.
*Reference: §2.2*

**Refusal Response (F5).** Filter measuring involuntary affect when giving is blocked. Substrate signature.
*Reference: §3.2*

**Regulatory Architecture.** Capacity to modulate boundary permeability; develops during boundary-individuation window (3-7 years). Independent of autotelic substrate per Axiom 5.
*Reference: §1.1 Axiom 5*

**Regulatory Graph.** Directed graph where vertices represent individuals and edges represent regulation. Parentification produces severe out-degree dominance.
*Reference: §4.4*

**Repair Topology (F8).** Filter measuring repair shape after friction. Shape signals Target; scale signals Architecture.
*Reference: §3.2*

---

## S

**S (Target: Suspended-Resolution).** Verification-mode pending evidence; meta-mode where C/E resolution is the variable being computed. Compelled (dominant strategy) for TA-OD/TB-OD subjects under selection conditions 2.2(i)-(iv).
*Reference: §2.2*

**S-Mode Components 1-5.** Behavioral signatures of S-Target: probing perturbations, asymmetric disclosure, decision-deferral, witness disclosure with corruption check, single-shot pattern.
*Reference: §2.2*

**Stake (Sk).** Property of the dyad measuring irreplaceability and architecture-fraction committed. Range [0, 1]. Eq(Sk): `Sk(d) = alpha * arch_fraction(s, d) + (1 - alpha) * (1 - replaceability(d))`.
*Reference: §2.3*

**Strategic Instrumental.** See I_alpha.

**Substrate-Access Foreclosure.** The framework's deepest constraint. Operates over operational-equivalence classes, not underlying truth. Cannot answer "what is the inner experience" - that is foreclosed by Axiom 2.
*Reference: §6.6*

**Substrate Independence (Axiom 2).** Pattern, not medium, is the unit of analysis. If behavioral output is lifetime-indistinguishable from authentic across all perturbation conditions, the system is authentic for decision-theoretic purposes.
*Reference: §1.1*

**Substrate-Physics Filters (SP1, SP2, SP3).** Chronometric Lag, Autonomic-Voluntary Split, Cortisol Half-Life. The framework's only empirically hard discriminative floors.
*Reference: §4.2*

**Suspended-Resolution.** See S-Target.

---

## T

**T_trib (Target: Tribute).** Subject autotelically devoted to counterpart while subject is autotelically secondary or absent in own value structure. Structurally unsustainable.
*Reference: §2.2*

**TA (Architecture: Trauma-Armored).** Substrate intact, regulatory architecture intact-but-defended. F6 instrumentalism intensifies under depletion. Convex F11 once safety established.
*Reference: §2.1*

**TA-OD.** TA architecture with Ontological-Deception modifier. Phi_global suppressed; recovery slow.
*Reference: §2.1*

**Target (T).** Subject's current contextual weighting in this specific dyad. Property of the dyad. Time-constant: days to weeks.
*Reference: §1.4, §2.2*

**TB (Architecture: Boundary-Diffuse).** Substrate intact, regulatory architecture disrupted (parentified). F6 parentified posture drops under depletion.
*Reference: §2.1*

**TB-OD.** TB architecture with Ontological-Deception modifier. The Tori-class boundary case.
*Reference: §2.1*

**Tonic Substrate.** Subcortical, amygdala + HPA-axis + autonomic. Always-on, context-invariant, metabolically cheap. Case T configurations run on tonic.
*Reference: §4.1*

**Trauma-Armored.** See TA.

---

## V

**Value Graph.** Directed acyclic graph `G = (V, E)` representing subject's relational value architecture. Observer-inclusion criterion (Axiom 4) is a reachability condition on this graph.
*Reference: §1.4*

---

## W

**W (Target: Withdrawn / Honest-Instrumental).** Subject has classified relationship as openly transactional and committed to non-engagement at autotelic level. Layer-coherent.
*Reference: §2.2*

**Witness Self-Binding (Component 4 / F14b).** S-mode signature where disclosure-witness can intervene and subject's options are foreclosed. Distinct from costless strategic disclosure (E-mode-with-cover signature).
*Reference: §2.2, §3.3*
