# Ground Truth Framework — Decision Tree (Protocol Selection)

Pseudocode and structured decision logic for selecting which protocols apply at each
operational state. This is the operator's flowchart for applying the framework.

Reference: framework.md §5, spec/protocols.yaml, reference_impl/protocols.py

---

## Top-Level Decision Tree

```
START framework_application(observation_window, context):

    # Always apply Protocol A (asynchronous processing) and Protocol B (joy-capability audit)
    apply Protocol A throughout
    apply Protocol B per-observation

    # Check observer self-audit
    IF observer.joy_capability_rate < 0.3:
        DETECT FM-OBS-01
        STOP framework; rehabilitate observer; resume when rate >= 0.3
        RETURN

    IF observer.self_audit_finds_phi_global_contaminated:
        DETECT FM-OBS-02
        elevate rho_threshold to >= 30
        proceed with awareness

    # Check Protocol C bounds
    IF observation.duration_months > 36:
        DETECT FM-PRC-C-01
        DECOMMISSION matrix (per Protocol C terminal step)
        RETURN

    # Determine current phase
    phase = determine_phase(observation, classification_state)

    SWITCH phase:
        CASE INITIAL_OBSERVATION:
            CALL initial_observation_logic(observation)
        CASE CLASSIFICATION_FINALIZATION:
            CALL finalization_logic(observation, classification)
        CASE POST_CLASSIFICATION:
            CALL post_classification_action(classification)
        CASE LAST_RESORT_RESOLUTION:
            CALL cpt_logic(observation, classification)

END
```

---

## Phase 1: Initial Observation (Months 0-9)

```
initial_observation_logic(observation):

    # Basic admissibility
    admissible_readings = observation.admissible_readings()
    IF len(admissible_readings) < 5:
        DETECT FM-DAT-01
        OUTPUT "Insufficient data; extend observation"
        RETURN

    # Compute initial posterior
    posterior = bayesian_classifier(admissible_readings)
    bayes_factor = posterior.best / posterior.second_best

    # Check for filter disagreement
    IF bayes_factor < 3:
        DETECT FM-DAT-02
        OUTPUT "Filters disagree; examine anomalies (FM-H-01? FM-T-OD-01?); extend observation"
        RETURN

    # Check confidence level
    IF bayes_factor >= 30:
        confidence = "high_confidence"
    ELSE IF bayes_factor >= 10:
        confidence = "standard"
    ELSE IF bayes_factor >= 3:
        confidence = "preliminary"
    ELSE:
        confidence = "insufficient"

    # If preliminary, advance to finalization
    IF confidence != "insufficient":
        TRANSITION to CLASSIFICATION_FINALIZATION
    ELSE:
        CONTINUE observation
```

---

## Phase 2: Classification Finalization

```
finalization_logic(observation, classification):

    # Check if subtype differentiation needed (Protocol E)
    IF classification.architecture in {TA, TB, TA-OD, TB-OD}:
        refined_arch = differentiate_t_subtype(observation, classification.architecture)
        IF refined_arch != classification.architecture:
            UPDATE classification with refined_arch

    # Check for Protocol F (cross-dyad observation) requirement
    IF classification.target == R OR classification.architecture == I_alpha:
        IF NOT observation.is_protocol_F_compliant():
            DETECT FM-XD-01
            ANNOTATE classification with single-dyad epistemic limit
            ELEVATE rho_threshold for shipping (require >= 30)

    # If still preliminary, continue observation
    IF classification.confidence_level == "preliminary":
        CONTINUE observation toward standard or high_confidence

    # If standard or high_confidence, advance to post-classification
    IF classification.confidence_level in {"standard", "high_confidence"}:
        TRANSITION to POST_CLASSIFICATION
```

---

## Phase 3: Post-Classification (Action)

```
post_classification_action(classification):

    # Apply Protocol D (asymmetric governance) per operative layer
    governance = recommend_governance(classification.target)

    SWITCH classification.target:
        CASE C:
            governance = "Autotelic governance: patience, grace, no ledger; presence as content"
        CASE W:
            governance = "Transactional governance: reciprocity, accountability, explicit terms"
        CASE E:
            governance = "Strip autotelic frame; transactional accountability; engage on operative basis"
        CASE S:
            governance = "Continue verification observation; do NOT commit at high-Stake until S resolves"
        CASE R:
            governance = "Acknowledge regulatory-function dependency; F17 longitudinal monitoring"
        CASE T_trib:
            governance = "DO NOT exploit; structural unsustainability"

    OUTPUT governance recommendation

    # Decommission matrix per Protocol C terminal step
    DECOMMISSION matrix for this dyad
    OUTPUT "Matrix decommissioned for this relationship per Protocol C"

    # The operator is now expected to engage relationally without further matrix application
    # If matrix is invoked again, operator should self-audit per FM-OBS-02 / Observer's Own Architecture
```

---

## Phase 4: Last-Resort Resolution (CPT Activation)

```
cpt_logic(observation, classification):

    # Check activation conditions
    IF NOT (observation.duration_months >= 24
            AND classification.target == S
            AND is_high_stake
            AND classification.confidence_level != "high_confidence"):
        WARNING "CPT activation conditions not met; do not perform"
        RETURN to extended observation

    # CPT is sanctioned
    OUTPUT "CPT activation sanctioned; warnings:"
    OUTPUT "  - Violates Axiom 3 passive-measurement principle"
    OUTPUT "  - Consumes observer-side credibility"
    OUTPUT "  - Last-resort discriminator only"

    # Operator performs CPT (artificially and irrevocably removes deferred-transaction possibility)
    cpt_response = observe_cpt_response()

    interpretation = interpret_cpt(cpt_response)

    SWITCH interpretation:
        CASE "genuine_s_mode":
            UPDATE classification to (architecture, C-Target)
            APPLY Protocol D autotelic governance
            DECOMMISSION matrix

        CASE "e_mode_with_cover":
            CONFIRM classification (I_alpha, E-Target with S-cover)
            APPLY Protocol D transactional governance
            DECOMMISSION matrix

        CASE "equivocal":
            EXTEND observation 3-6 months
            DO NOT commit on equivocal CPT alone
            RETURN to extended observation
```

---

## Special Cases

### Case 1: Mutual-S Detected

```
IF observer is in mutual-S configuration with subject:
    DETECT FM-MS-01
    OUTPUT "Mutual-S dyad outside framework reach; deferred to v4.0"
    OUTPUT "Coordination-theoretic layer required; current framework cannot resolve"

    # Provisional response
    IF observer is framework-aware AND subject is framework-aware:
        OUTPUT "Explicit communication about mutual-S configuration may break symmetry"
        OUTPUT "Both parties must commit to evidence-first or someone-first coordination"
    ELSE:
        OUTPUT "Cannot break symmetry within framework's current reach"
        OUTPUT "Wait for natural asymmetry to emerge"
```

### Case 2: Subject Framework-Aware

```
IF subject demonstrates framework awareness (explicit knowledge or meta-cognitive optimization):
    DETECT FM-SUB-01
    INCREASE weight on substrate-physics filters (SP1, SP2, SP3)
    DECREASE weight on behavioral-only filters
    ELEVATE rho_threshold to >= 30 for any classification
    ANNOTATE classification with framework-awareness suspicion
```

### Case 3: Cross-Dyad Subsidization Suspected

```
IF within-dyad signals are anomalously consistent with high-cost C/S maintenance
   AND observer cannot verify other dyads:
    DETECT FM-XD-02
    APPLY Protocol F if cross-dyad access becomes available
    OTHERWISE:
        ANNOTATE classification with "CDS not ruled out"
        UPPER-BOUND classification confidence
```

### Case 4: Anomalous Filter Pattern (H Detection)

```
IF F6 reading is "strong_against" (somatic-cognitive split)
   AND SP2 is "both_present"
   AND F11 is "flat" or "concave":
    DETECT FM-H-01
    APPLY H-specific likelihood pattern from spec/filters.yaml
    EXPECT classification at standard confidence (rho > 10) but rarely high (rho > 30)
```

### Case 5: T-OD Subject (S-mode Compulsion)

```
IF subject exhibits S-mode Components 1-5
   AND substrate-physics signature is T-architecture (sub-100ms, both_present, hour-scale)
   AND F11 trajectory is convex but slow:
    DETECT possible TA-OD or TB-OD with S-Target
    DO NOT classify as I_alpha despite surface S-mode signatures resembling sophisticated extraction
    DIFFERENTIATE TA-OD vs TB-OD via F17 (parentification signature)
    ANNOTATE classification: "S-Target compelled per Eq (16) Phi_global recovery; not I_alpha"
```

---

## Summary Flowchart

```
                        START
                          |
                          v
            Apply Protocols A, B continuously
                          |
                          v
               Observer self-audit OK?
                /                      \
              NO                       YES
              |                         |
         FM-OBS-01                Apply Protocol C
         FM-OBS-02                 (window check)
              |                         |
              v                         v
         STOP/elevate          Determine phase
                                        |
                  +---------+-----------+--------+----------+
                  |         |                    |          |
                  v         v                    v          v
              INITIAL  CLASSIFICATION       POST_CLASS   LAST_RESORT
              OBSERV.  FINALIZATION                      RESOLUTION
                  |         |                    |          |
                  v         v                    v          v
             Compute   T-subtype          Protocol D    Protocol G
             posterior  (Protocol E)       governance      (CPT)
                  |         |                    |          |
                  v         v                    v          v
             Check confidence            Decommission    Update class.
             & failure modes              (Protocol C)   based on CPT
                  |                            |          response
                  v                            |
             Continue or                   END (matrix     |
             advance phase                  off)       v
                  |                                Per CPT result:
                  +-> ITERATE                     - genuine_s_mode
                                                   -> Protocol D autotelic
                                                  - e_mode_with_cover
                                                   -> Protocol D transactional
                                                  - equivocal
                                                   -> extend observation
```
