"""
Ground Truth Framework — Reference Implementation
Protocol Selection Logic

Implements the protocol-selection decision tree from spec/protocols.yaml.
Protocols govern HOW the framework is applied; this module determines
which protocol applies in a given operational state.

Reference: framework.md §5
"""

from __future__ import annotations
from dataclasses import dataclass
from enum import Enum
from typing import Optional

from data_types import (
    Architecture, Target, ObservationWindow, Classification, FailureModeDetection,
)


class Protocol(str, Enum):
    """The 7 protocols A-G plus framework-level operations."""
    A = "A"  # Asynchronous Processing (always active)
    B = "B"  # Autotelic Control Audit (per-observation)
    C = "C"  # Bounded Window
    D = "D"  # Asymmetric Governance
    E = "E"  # T Subtype Differentiation
    F = "F"  # Cross-Dyad Observation
    G = "G"  # Counter-Perturbation Test (CPT)


class Phase(str, Enum):
    """Operational phases of framework application."""
    INITIAL_OBSERVATION = "initial_observation"
    CLASSIFICATION_FINALIZATION = "classification_finalization"
    POST_CLASSIFICATION = "post_classification"
    CROSS_DYAD_CHECK = "cross_dyad_check"
    LAST_RESORT_RESOLUTION = "last_resort_resolution"
    DECOMMISSION = "decommission"


@dataclass
class ProtocolDecision:
    """Output of protocol-selection logic."""
    phase: Phase
    active_protocols: list[Protocol]
    rationale: str
    next_action: str
    warnings: list[str]


def select_protocol(
    observation: ObservationWindow,
    classification: Optional[Classification] = None,
    is_high_stake: bool = False,
    has_cross_dyad_access: bool = False,
    s_mode_persistent_months: float = 0.0,
) -> ProtocolDecision:
    """Determine which protocols apply at the current operational state.

    Args:
        observation: Current observation window
        classification: Current classification if any (None = pre-classification)
        is_high_stake: Whether the case is high-Stake (affects rho_threshold)
        has_cross_dyad_access: Whether observer has access to multiple dyads
        s_mode_persistent_months: How long S-mode has persisted without resolution

    Returns:
        ProtocolDecision with current phase, active protocols, and next action
    """
    warnings = []

    # Check Protocol C compliance first
    if observation.duration_months > 36:
        warnings.append(
            "Observation window exceeds Protocol C maximum (36 months); decommission matrix"
        )
        return ProtocolDecision(
            phase=Phase.DECOMMISSION,
            active_protocols=[],
            rationale="Protocol C bounded-window violation; matrix must be decommissioned",
            next_action="Decommission matrix immediately; matrix is now operating as relational pathology per §5.3",
            warnings=warnings,
        )

    # Pre-classification phase
    if classification is None or not classification.is_shippable:
        active = [Protocol.A, Protocol.B]
        if observation.duration_months > 9:
            active.append(Protocol.C)  # extending into longitudinal window
        if has_cross_dyad_access:
            active.append(Protocol.F)
        return ProtocolDecision(
            phase=Phase.INITIAL_OBSERVATION,
            active_protocols=active,
            rationale="Classification not yet finalized; continue observation under Protocol A and B",
            next_action="Continue observation; apply Protocol B per-event; extend window if needed",
            warnings=warnings,
        )

    # Classification finalization
    if classification.confidence_level == "preliminary":
        active = [Protocol.A, Protocol.B]
        if classification.architecture in {Architecture.TA, Architecture.TB,
                                            Architecture.TA_OD, Architecture.TB_OD}:
            active.append(Protocol.E)  # T subtype differentiation
        return ProtocolDecision(
            phase=Phase.CLASSIFICATION_FINALIZATION,
            active_protocols=active,
            rationale="Preliminary classification; finalize via subtype differentiation if Case T",
            next_action="Continue observation to standard or high-confidence threshold",
            warnings=warnings,
        )

    # Cross-dyad check for relevant cases
    if classification.target == Target.R or classification.architecture == Architecture.I_ALPHA:
        if not has_cross_dyad_access:
            warnings.append(
                "Cross-dyad observation breadth not available; classification bounded by single-dyad epistemic limit per §6.2"
            )

    # High-stake S-mode persistent past 24 months
    if (
        classification.target == Target.S
        and s_mode_persistent_months > 24
        and is_high_stake
        and classification.confidence_level != "high_confidence"
    ):
        warnings.append(
            "S-mode persistent past 24 months at high Stake; consider Protocol G (CPT) per §3.5"
        )
        return ProtocolDecision(
            phase=Phase.LAST_RESORT_RESOLUTION,
            active_protocols=[Protocol.A, Protocol.B, Protocol.G],
            rationale="S-mode unresolved past 24 months at high Stake; CPT activation conditions met",
            next_action="Consider Protocol G (CPT) - last-resort discriminator; warning: violates Axiom 3 and consumes observer credibility",
            warnings=warnings,
        )

    # Post-classification: apply Protocol D (asymmetric governance) and decommission per Protocol C
    return ProtocolDecision(
        phase=Phase.POST_CLASSIFICATION,
        active_protocols=[Protocol.D, Protocol.C],
        rationale="Classification finalized; apply asymmetric governance and decommission matrix",
        next_action=f"Apply Protocol D: {_governance_recommendation(classification)}; then decommission matrix per Protocol C",
        warnings=warnings,
    )


def _governance_recommendation(classification: Classification) -> str:
    """Recommend governance per Protocol D given classification."""
    if classification.target == Target.C:
        return "Autotelic governance: patience, grace, no ledger; presence as the content"
    elif classification.target == Target.W:
        return "Transactional governance: reciprocity, accountability, explicit terms"
    elif classification.target == Target.E:
        return "Strip the autotelic frame; apply transactional accountability; engage on actual operative basis"
    elif classification.target == Target.S:
        return "Continue verification observation; do NOT commit at high-Stake until S resolves"
    elif classification.target == Target.R:
        return "Acknowledge regulatory-function dependency; do NOT mistake for autotelic; F17 longitudinal monitoring"
    elif classification.target == Target.T_TRIB:
        return "DO NOT exploit T_trib configuration even when offered; structural unsustainability per §2.5"
    return "Apply standard governance per classification"


# ============================================================================
# T-subtype differentiation (Protocol E)
# ============================================================================

def differentiate_t_subtype(
    observation: ObservationWindow,
    architecture: Architecture,
) -> Architecture:
    """Run Protocol E to disambiguate Case T subtype.

    Examines F1 covariance, F6 depletion direction, F15-Eq parentification ratio,
    and OD modifier signals (F12 asymmetric disclosure, F14b witness self-binding).

    Returns:
        Refined architecture (TA, TB, TA_OD, or TB_OD)
    """
    if architecture not in {Architecture.TA, Architecture.TB,
                            Architecture.TA_OD, Architecture.TB_OD}:
        return architecture

    # Check for parentification signature (F15-Eq via F17)
    f17_readings = observation.filter_readings("F17")
    has_parentification = any(
        r.level.value in {"strong_for", "weak_for"} for r in f17_readings
    )

    # Check for OD modifier (F12 asymmetric + F14b witness self-binding)
    f12_readings = observation.filter_readings("F12")
    f14b_readings = observation.filter_readings("F14b")
    has_asymmetric_disclosure = any(
        r.notes and "asymmetric" in r.notes.lower() for r in f12_readings
    )
    has_witness_self_binding = any(
        r.level.value == "strong_for" for r in f14b_readings
    )

    has_od = has_asymmetric_disclosure and has_witness_self_binding

    # Refine
    if has_parentification:
        return Architecture.TB_OD if has_od else Architecture.TB
    else:
        return Architecture.TA_OD if has_od else Architecture.TA


# ============================================================================
# CPT (Protocol G) response analysis
# ============================================================================

@dataclass
class CPTResponse:
    """Subject's response to a Counter-Perturbation Test."""
    f2_relief_observed: bool          # within 24-72 hours
    f10_spike_observed: bool          # within 0-48 hours
    f12_depth_increase: bool          # disclosure depth advances
    subject_disengaged: bool          # within 1-4 weeks
    subject_renegotiating: bool       # attempts to recover foreclosed option


def interpret_cpt(response: CPTResponse) -> str:
    """Interpret CPT response per §3.5.

    Returns one of:
        "genuine_s_mode" - F2 relief, depth increase
        "e_mode_with_cover" - F10 spike, disengagement or renegotiation
        "equivocal" - mixed signals; extend observation
    """
    if response.f2_relief_observed and response.f12_depth_increase:
        return "genuine_s_mode"

    if response.f10_spike_observed and (response.subject_disengaged or response.subject_renegotiating):
        return "e_mode_with_cover"

    return "equivocal"


def cpt_action(interpretation: str) -> str:
    """Recommend action based on CPT interpretation."""
    if interpretation == "genuine_s_mode":
        return "Update classification to (Architecture, C-Target); proceed with Protocol D autotelic governance"
    elif interpretation == "e_mode_with_cover":
        return "Confirm classification (I_alpha, E with S-cover); apply Protocol D transactional governance; decommission matrix"
    elif interpretation == "equivocal":
        return "Extend observation 3-6 months; do NOT commit on equivocal CPT alone"
    return "Unknown interpretation"
