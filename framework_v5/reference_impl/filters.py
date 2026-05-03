"""
Ground Truth Framework — Reference Implementation
Filter Likelihoods

Provides P(filter_reading | architecture, target) for the Bayesian classifier.
Uses ordinal-default likelihood ratios with substrate-physics overrides.

Reference: framework.md §3, spec/filters.yaml
"""

from __future__ import annotations
from data_types import Architecture, Target, ReadingLevel, FilterReading

# ============================================================================
# Default ordinal likelihood ratios (per spec/thresholds.yaml)
# ============================================================================

ORDINAL_LIKELIHOOD_RATIOS = {
    ReadingLevel.STRONG_FOR: 5.0,
    ReadingLevel.WEAK_FOR: 2.0,
    ReadingLevel.NEUTRAL: 1.0,
    ReadingLevel.WEAK_AGAINST: 0.5,
    ReadingLevel.STRONG_AGAINST: 0.2,
}


# ============================================================================
# Filter discrimination structure
# ============================================================================
# Each filter has a "discrimination direction" - what it primarily discriminates.
# Likelihood ratios are computed relative to the filter's primary discrimination.

# Architecture-discriminating filters (substrate probes)
# Format: filter_id -> {architecture: relative_likelihood_at_strong_for}
# Other reading levels scale via ORDINAL_LIKELIHOOD_RATIOS
ARCHITECTURE_FILTER_DIRECTIONS = {
    "F2": {
        # F2 = Affect in Autotelic Void
        # "strong_for" = wide register calm; favors A, T-architectures; against I-architectures
        Architecture.A: 5.0,
        Architecture.TA: 3.0,
        Architecture.TB: 2.0,
        Architecture.TA_OD: 2.0,
        Architecture.TB_OD: 2.0,
        Architecture.I_ALPHA: 0.1,
        Architecture.I_BETA: 0.1,
        Architecture.H: 0.2,
        Architecture.D: 0.3,
    },
    "F5": {
        # F5 = Refusal Response
        # "strong_for" = brief disappointment then warm acceptance; favors autotelic
        Architecture.A: 5.0,
        Architecture.TA: 3.0,
        Architecture.TB: 4.0,
        Architecture.TA_OD: 3.0,
        Architecture.TB_OD: 3.0,
        Architecture.I_ALPHA: 0.5,
        Architecture.I_BETA: 0.3,
        Architecture.H: 0.3,
        Architecture.D: 0.5,
    },
    "F6": {
        # F6 = Cognitive Entropy under Depletion
        # "strong_for" depends on depletion DIRECTION; this is the discriminator
        # Special handling: F6 has 5 categorical readings rather than ordinal
        # Use F6 with custom resolver below
        Architecture.A: 1.0,  # placeholder - use custom resolver
        Architecture.TA: 1.0,
        Architecture.TB: 1.0,
        Architecture.TA_OD: 1.0,
        Architecture.TB_OD: 1.0,
        Architecture.I_ALPHA: 1.0,
        Architecture.I_BETA: 1.0,
        Architecture.H: 1.0,
        Architecture.D: 1.0,
    },
    "F7": {
        # F7 = Epistemic Exhaust (memory frame)
        # "strong_for" = atmospheric dominant
        Architecture.A: 5.0,
        Architecture.TA: 3.0,
        Architecture.TB: 3.0,
        Architecture.TA_OD: 2.0,
        Architecture.TB_OD: 2.0,
        Architecture.I_ALPHA: 0.1,
        Architecture.I_BETA: 0.05,
        Architecture.H: 0.3,
        Architecture.D: 0.2,
    },
    "F9": {
        # F9 = Attentional Breadth
        # "strong_for" = breadth dominant (non-instrumental personal details retained)
        Architecture.A: 5.0,
        Architecture.TA: 3.0,
        Architecture.TB: 3.0,
        Architecture.TA_OD: 2.0,
        Architecture.TB_OD: 2.0,
        Architecture.I_ALPHA: 0.1,
        Architecture.I_BETA: 0.05,
        Architecture.H: 0.3,
        Architecture.D: 0.2,
    },
    "F11": {
        # F11 = Investment Return Curvature
        # "strong_for" = convex strong (kappa >> kappa_noise positive)
        Architecture.A: 0.5,    # A near ceiling; flat is more likely
        Architecture.TA: 5.0,
        Architecture.TB: 5.0,
        Architecture.TA_OD: 4.0,
        Architecture.TB_OD: 4.0,
        Architecture.I_ALPHA: 0.05,
        Architecture.I_BETA: 0.02,
        Architecture.H: 0.1,
        Architecture.D: 0.02,
    },
    "F17": {
        # F17 = Regulatory Circuit Acquisition
        # "strong_for" = acquiring strongly (in_degree increasing)
        Architecture.A: 2.0,
        Architecture.TA: 0.5,
        Architecture.TB: 5.0,
        Architecture.TA_OD: 3.0,
        Architecture.TB_OD: 4.0,
        Architecture.I_ALPHA: 0.3,
        Architecture.I_BETA: 0.3,
        Architecture.H: 0.3,
        Architecture.D: 0.3,
    },
}

# Substrate-physics filters (highest reliability, derived from biology)
SUBSTRATE_PHYSICS_FILTER_DIRECTIONS = {
    "SP1": {
        # SP1 = Chronometric Lag
        # "strong_for" = sub-100ms (genuine subcortical)
        Architecture.A: 5.0,
        Architecture.TA: 5.0,
        Architecture.TB: 5.0,
        Architecture.TA_OD: 4.0,
        Architecture.TB_OD: 4.0,
        Architecture.I_ALPHA: 0.05,
        Architecture.I_BETA: 0.05,
        Architecture.H: 0.5,
        Architecture.D: 0.3,
    },
    "SP2": {
        # SP2 = Autonomic-Voluntary Split
        # "strong_for" = both present (autonomic alongside voluntary)
        Architecture.A: 5.0,
        Architecture.TA: 5.0,
        Architecture.TB: 5.0,
        Architecture.TA_OD: 4.0,
        Architecture.TB_OD: 4.0,
        Architecture.I_ALPHA: 0.05,
        Architecture.I_BETA: 0.05,
        Architecture.H: 0.5,
        Architecture.D: 0.3,
    },
    "SP3": {
        # SP3 = Cortisol Half-Life
        # "strong_for" = hour-scale persistence
        Architecture.A: 5.0,
        Architecture.TA: 4.0,
        Architecture.TB: 4.0,
        Architecture.TA_OD: 3.0,
        Architecture.TB_OD: 3.0,
        Architecture.I_ALPHA: 0.1,
        Architecture.I_BETA: 0.05,
        Architecture.H: 0.5,
        Architecture.D: 0.3,
    },
}

# Target-discriminating filters (target probes)
# Format: filter_id -> {target: relative_likelihood_at_strong_for}
TARGET_FILTER_DIRECTIONS = {
    "F1": {
        # F1 = Engagement-Utility Covariance
        # "strong_for" = engagement covaries with utility -> strong evidence for E
        Target.E: 5.0,
        Target.C: 0.05,
        Target.S: 0.5,
        Target.W: 0.5,
        Target.R: 0.5,
        Target.T_TRIB: 0.3,
    },
    "F3": {
        # F3 = Post-Connection Decay Rate
        # "strong_for" = slow decay -> evidence for C, against E
        Target.C: 5.0,
        Target.E: 0.05,
        Target.S: 1.0,
        Target.W: 0.3,
        Target.R: 1.0,
        Target.T_TRIB: 2.0,
    },
    "F4": {
        # F4 = Cross-Relationship Selectivity
        # "strong_for" = strong utility organization -> E
        Target.E: 5.0,
        Target.C: 0.1,
        Target.S: 0.5,
        Target.W: 1.0,
        Target.R: 1.0,
        Target.T_TRIB: 0.3,
    },
    "F10": {
        # F10 = Friction Tax (current Target)
        # "strong_for" = autotelic appreciation -> C
        Target.C: 5.0,
        Target.E: 0.05,
        Target.S: 1.0,
        Target.W: 0.3,
        Target.R: 0.5,
        Target.T_TRIB: 2.0,
    },
}

# Dual-purpose filters (both architecture and target signal)
# These are simplified to target-direction here; architecture component handled separately
DUAL_PURPOSE_FILTER_TARGET_DIRECTIONS = {
    "F8": {
        # F8 = Repair Topology
        # "strong_for" = mutual dignified resolution -> C
        Target.C: 5.0,
        Target.E: 0.1,
        Target.S: 1.0,
        Target.W: 0.5,
        Target.R: 0.5,
        Target.T_TRIB: 1.0,
    },
    "F12": {
        # F12 = Disclosure Hierarchy
        # "strong_for" = progressive to load-bearing -> C
        Target.C: 5.0,
        Target.E: 0.05,
        Target.S: 0.5,
        Target.W: 0.5,
        Target.R: 0.5,
        Target.T_TRIB: 0.3,
    },
}

# Target-trajectory filters for S-mode resolution
# IMPORTANT: These filters detect S-mode signatures (the active verification state).
# A "strong_for" reading means the subject is CURRENTLY in S-Target.
# Subjects in C-Target do not run probing operations, so probing-related strong_for
# signals are highly diagnostic for S vs C.
TARGET_TRAJECTORY_FILTERS = {
    "F13a": {
        # "strong_for" decreasing kappa_bid -> currently probing (S), with trajectory toward C
        # Subject in C-target wouldn't be running probing operations at all
        Target.S: 5.0,
        Target.C: 0.3,  # incompatible with active probing
        Target.E: 0.1,
    },
    "F13b": {
        Target.S: 5.0,
        Target.C: 0.3,
        Target.E: 0.3,
    },
    "F14a": {
        # "strong_for" progressive disclosure -> active S-mode (progression of depth toward C)
        # Less diagnostic than F13a because progressive disclosure is also possible in established C
        Target.S: 4.0,
        Target.C: 1.5,
        Target.E: 0.1,
    },
    "F14b": {
        # "strong_for" witness self-binding -> diagnostic for genuine S-mode
        # Subject in C wouldn't need witness self-binding for verification
        Target.S: 5.0,
        Target.C: 0.5,
        Target.E: 0.1,
    },
    "F15a": {
        # "strong_for" consistent non-consumption -> current S or C (neither consume implied transactions)
        # This filter doesn't strongly distinguish S from C
        Target.S: 4.0,
        Target.C: 3.0,
        Target.E: 0.1,
    },
    "F15b": {
        # "strong_for" uniform non-consumption -> current S or C
        Target.S: 4.0,
        Target.C: 3.0,
        Target.E: 0.1,
    },
    "F16": {
        # "strong_for" anti-utility correlation in PROBE selection -> diagnostic for genuine S-mode
        # Subject in C wouldn't have probe-targets at all (no probing)
        Target.S: 5.0,
        Target.C: 0.3,
        Target.E: 0.1,
    },
}


# ============================================================================
# Likelihood computation
# ============================================================================

def likelihood_architecture(reading: FilterReading, arch: Architecture) -> float:
    """P(reading | architecture).

    For architecture-discriminating filters, applies the filter's discrimination direction.
    For target-only filters, returns 1.0 (uninformative for architecture).
    For dual-purpose filters with OD-relevant signatures, applies categorical detection.

    Reference: framework.md §3.7, Eq 19
    """
    fid = reading.filter_id

    # F6 special handling: depletion direction has 5 categorical readings
    if fid == "F6":
        return _f6_likelihood(reading, arch)

    # F8 dual-purpose: STRONG_AGAINST signals "no_repair" which is W or D signature
    if fid == "F8":
        return _f8_architecture_likelihood(reading, arch)

    # F11 special handling: convex/flat/concave have distinct architecture implications
    if fid == "F11":
        return _f11_likelihood(reading, arch)

    # F12 dual-purpose: WEAK_FOR signals asymmetric disclosure (OD modifier signature)
    if fid == "F12":
        return _f12_architecture_likelihood(reading, arch)

    # F14b: witness self-binding (strong_for) is OD-recovery-attempt signature
    if fid == "F14b":
        return _f14b_architecture_likelihood(reading, arch)

    # Architecture-direct filters
    if fid in ARCHITECTURE_FILTER_DIRECTIONS:
        base = ARCHITECTURE_FILTER_DIRECTIONS[fid].get(arch, 1.0)
        return _scale_by_reading_level(base, reading.level)

    # Substrate-physics filters
    if fid in SUBSTRATE_PHYSICS_FILTER_DIRECTIONS:
        base = SUBSTRATE_PHYSICS_FILTER_DIRECTIONS[fid].get(arch, 1.0)
        return _scale_by_reading_level(base, reading.level)

    # Filter doesn't discriminate architecture; uninformative
    return 1.0


def _f12_architecture_likelihood(reading: FilterReading, arch: Architecture) -> float:
    """F12 disclosure hierarchy - architecture component.

    Reading levels (categorical interpretation per spec/filters.yaml):
        STRONG_FOR -> progressive to load-bearing (favors A, TA, TB)
        WEAK_FOR -> asymmetric high surface low deep (OD signature; favors TA-OD, TB-OD)
        NEUTRAL -> shallow plateau or strategic
        WEAK_AGAINST -> strategic disclosure (E with cover; favors I_alpha, H)
        STRONG_AGAINST -> no disclosure (W or D)
    """
    f12_table = {
        ReadingLevel.STRONG_FOR: {
            # progressive to load-bearing - favors autotelic architectures
            Architecture.A: 5.0, Architecture.TA: 4.0, Architecture.TB: 3.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 0.05, Architecture.I_BETA: 0.02,
            Architecture.H: 0.2, Architecture.D: 0.1,
        },
        ReadingLevel.WEAK_FOR: {
            # asymmetric high surface low deep - OD signature
            Architecture.A: 0.5, Architecture.TA: 1.0, Architecture.TB: 2.0,
            Architecture.TA_OD: 4.0, Architecture.TB_OD: 5.0,  # strongly diagnostic for OD
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 0.5,
            Architecture.H: 1.0, Architecture.D: 0.3,
        },
        ReadingLevel.NEUTRAL: {
            # shallow plateau
            Architecture.A: 0.3, Architecture.TA: 1.0, Architecture.TB: 1.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 3.0, Architecture.I_BETA: 4.0,
            Architecture.H: 3.0, Architecture.D: 2.0,
        },
        ReadingLevel.WEAK_AGAINST: {
            # strategic disclosure - E with cover signature
            Architecture.A: 0.1, Architecture.TA: 0.3, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 4.0, Architecture.I_BETA: 3.0,
            Architecture.H: 4.0, Architecture.D: 1.0,
        },
        ReadingLevel.STRONG_AGAINST: {
            # no disclosure - W or D
            Architecture.A: 0.5, Architecture.TA: 1.0, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 1.0,
            Architecture.H: 1.0, Architecture.D: 3.0,
        },
    }
    return f12_table.get(reading.level, {}).get(arch, 1.0)


def _f8_architecture_likelihood(reading: FilterReading, arch: Architecture) -> float:
    """F8 Repair Topology - architecture component.

    Reading levels (categorical interpretation per spec/filters.yaml):
        STRONG_FOR -> mutual dignified resolution -> A, TA, TB
        WEAK_FOR -> asymmetric subject-dignity protected (T_trib)
        NEUTRAL -> standard repair, no distinctive pattern
        WEAK_AGAINST -> asymmetric, transactional, or strategic
        STRONG_AGAINST -> no repair (W or D)
    """
    f8_table = {
        ReadingLevel.STRONG_FOR: {
            Architecture.A: 5.0, Architecture.TA: 4.0, Architecture.TB: 3.0,
            Architecture.TA_OD: 2.0, Architecture.TB_OD: 2.0,
            Architecture.I_ALPHA: 0.5, Architecture.I_BETA: 0.05,
            Architecture.H: 0.3, Architecture.D: 0.1,
        },
        ReadingLevel.WEAK_FOR: {
            Architecture.A: 1.0, Architecture.TA: 2.0, Architecture.TB: 3.0,
            Architecture.TA_OD: 2.0, Architecture.TB_OD: 3.0,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 0.5,
            Architecture.H: 1.0, Architecture.D: 0.5,
        },
        ReadingLevel.NEUTRAL: {
            Architecture.A: 1.0, Architecture.TA: 1.0, Architecture.TB: 1.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 1.0,
            Architecture.H: 1.0, Architecture.D: 1.0,
        },
        ReadingLevel.WEAK_AGAINST: {
            # Asymmetric, transactional, or strategic repair - I signature
            Architecture.A: 0.3, Architecture.TA: 0.5, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 3.0, Architecture.I_BETA: 3.0,
            Architecture.H: 3.0, Architecture.D: 1.0,
        },
        ReadingLevel.STRONG_AGAINST: {
            # No repair - W or D signature
            Architecture.A: 0.1, Architecture.TA: 0.5, Architecture.TB: 0.3,
            Architecture.TA_OD: 0.3, Architecture.TB_OD: 0.3,
            Architecture.I_ALPHA: 1.5, Architecture.I_BETA: 2.0,
            Architecture.H: 1.5, Architecture.D: 4.0,
        },
    }
    return f8_table.get(reading.level, {}).get(arch, 1.0)


def _f11_likelihood(reading: FilterReading, arch: Architecture) -> float:
    """F11 Investment Return Curvature - architecture-specific likelihoods.

    Reading levels (categorical interpretation per spec/filters.yaml):
        STRONG_FOR -> convex_strong (kappa >> kappa_noise positive) -> Case T strong signature
        WEAK_FOR -> convex_moderate -> Case T moderate signature
        NEUTRAL -> flat (|kappa| < kappa_noise) -> A near ceiling OR I (no curvature)
        WEAK_AGAINST -> concave_moderate -> Case D moderate signature
        STRONG_AGAINST -> concave_strong -> Case D strong signature

    Critical: NEUTRAL distinguishes I (flat, no convex) from D (would be concave).
    The flat-vs-concave distinction is the I-vs-D discriminator at F11.
    """
    f11_table = {
        ReadingLevel.STRONG_FOR: {
            # Convex strong - clear T signature
            Architecture.A: 0.5, Architecture.TA: 5.0, Architecture.TB: 5.0,
            Architecture.TA_OD: 4.0, Architecture.TB_OD: 4.0,
            Architecture.I_ALPHA: 0.05, Architecture.I_BETA: 0.02,
            Architecture.H: 0.1, Architecture.D: 0.02,
        },
        ReadingLevel.WEAK_FOR: {
            # Convex moderate
            Architecture.A: 1.0, Architecture.TA: 4.0, Architecture.TB: 4.0,
            Architecture.TA_OD: 3.0, Architecture.TB_OD: 3.0,
            Architecture.I_ALPHA: 0.2, Architecture.I_BETA: 0.1,
            Architecture.H: 0.3, Architecture.D: 0.1,
        },
        ReadingLevel.NEUTRAL: {
            # Flat - A near ceiling OR I (no curvature in either direction)
            # Distinguished from D which would show concavity
            Architecture.A: 4.0, Architecture.TA: 1.0, Architecture.TB: 1.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 3.0, Architecture.I_BETA: 3.0,
            Architecture.H: 3.0, Architecture.D: 0.5,
        },
        ReadingLevel.WEAK_AGAINST: {
            # Concave moderate - D signature, weak I signal
            Architecture.A: 0.3, Architecture.TA: 0.5, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 1.0,
            Architecture.H: 1.5, Architecture.D: 4.0,
        },
        ReadingLevel.STRONG_AGAINST: {
            # Concave strong - clear D signature
            Architecture.A: 0.05, Architecture.TA: 0.2, Architecture.TB: 0.3,
            Architecture.TA_OD: 0.3, Architecture.TB_OD: 0.3,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 1.0,
            Architecture.H: 2.0, Architecture.D: 5.0,
        },
    }
    return f11_table.get(reading.level, {}).get(arch, 1.0)


def _f14b_architecture_likelihood(reading: FilterReading, arch: Architecture) -> float:
    """F14b witness self-binding - OD recovery attempt signature.

    Strong_for (witness self-binding present, independent) indicates the subject is
    actively engaging in S-mode verification AND has structurally outside witnesses.
    This is highly diagnostic for OD architectures attempting Phi_global recovery.
    """
    f14b_table = {
        ReadingLevel.STRONG_FOR: {
            # Witness self-binding independent - OD recovery signature
            Architecture.A: 1.0, Architecture.TA: 1.0, Architecture.TB: 2.0,
            Architecture.TA_OD: 4.0, Architecture.TB_OD: 5.0,  # diagnostic for OD
            Architecture.I_ALPHA: 0.1, Architecture.I_BETA: 0.05,
            Architecture.H: 0.1, Architecture.D: 0.1,
        },
        ReadingLevel.WEAK_FOR: {
            Architecture.A: 1.0, Architecture.TA: 1.0, Architecture.TB: 1.5,
            Architecture.TA_OD: 2.0, Architecture.TB_OD: 2.0,
            Architecture.I_ALPHA: 0.5, Architecture.I_BETA: 0.3,
            Architecture.H: 0.5, Architecture.D: 0.5,
        },
        ReadingLevel.NEUTRAL: {
            Architecture.A: 1.0, Architecture.TA: 1.0, Architecture.TB: 1.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 1.0, Architecture.I_BETA: 1.0,
            Architecture.H: 1.0, Architecture.D: 1.0,
        },
        ReadingLevel.WEAK_AGAINST: {
            # Costless/corrupted witness - E-with-cover signature
            Architecture.A: 0.5, Architecture.TA: 0.5, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 3.0, Architecture.I_BETA: 2.0,
            Architecture.H: 3.0, Architecture.D: 1.0,
        },
        ReadingLevel.STRONG_AGAINST: {
            # No witness disclosure
            Architecture.A: 1.0, Architecture.TA: 1.0, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.3, Architecture.TB_OD: 0.3,
            Architecture.I_ALPHA: 2.0, Architecture.I_BETA: 2.0,
            Architecture.H: 2.0, Architecture.D: 2.0,
        },
    }
    return f14b_table.get(reading.level, {}).get(arch, 1.0)


def likelihood_target(reading: FilterReading, target: Target) -> float:
    """P(reading | target).

    For target-discriminating filters, applies the filter's discrimination direction.
    For architecture-only filters, returns 1.0 (uninformative for target).
    """
    fid = reading.filter_id

    # F12 dual-purpose with categorical target component
    if fid == "F12":
        return _f12_target_likelihood(reading, target)

    # F8 dual-purpose with categorical target component
    if fid == "F8":
        return _f8_target_likelihood(reading, target)

    if fid in TARGET_FILTER_DIRECTIONS:
        base = TARGET_FILTER_DIRECTIONS[fid].get(target, 1.0)
        return _scale_by_reading_level(base, reading.level)

    if fid in DUAL_PURPOSE_FILTER_TARGET_DIRECTIONS:
        base = DUAL_PURPOSE_FILTER_TARGET_DIRECTIONS[fid].get(target, 1.0)
        return _scale_by_reading_level(base, reading.level)

    if fid in TARGET_TRAJECTORY_FILTERS:
        base = TARGET_TRAJECTORY_FILTERS[fid].get(target, 1.0)
        return _scale_by_reading_level(base, reading.level)

    return 1.0


def _f12_target_likelihood(reading: FilterReading, target: Target) -> float:
    """F12 Disclosure Hierarchy - target component.

    Categorical readings:
        STRONG_FOR -> progressive (C signature)
        WEAK_FOR -> asymmetric (S signature; OD recovery)
        NEUTRAL -> shallow plateau (E or persistent S)
        WEAK_AGAINST -> strategic disclosure (E with cover)
        STRONG_AGAINST -> no disclosure (W signature)
    """
    table = {
        ReadingLevel.STRONG_FOR: {Target.C: 5.0, Target.E: 0.05, Target.S: 0.5, Target.W: 0.3, Target.R: 0.5, Target.T_TRIB: 0.3},
        ReadingLevel.WEAK_FOR: {Target.C: 1.0, Target.E: 0.3, Target.S: 4.0, Target.W: 0.5, Target.R: 1.0, Target.T_TRIB: 1.0},
        ReadingLevel.NEUTRAL: {Target.C: 0.3, Target.E: 3.0, Target.S: 2.0, Target.W: 1.0, Target.R: 1.0, Target.T_TRIB: 1.0},
        ReadingLevel.WEAK_AGAINST: {Target.C: 0.1, Target.E: 4.0, Target.S: 0.5, Target.W: 1.0, Target.R: 1.0, Target.T_TRIB: 0.5},
        ReadingLevel.STRONG_AGAINST: {Target.C: 0.05, Target.E: 1.0, Target.S: 0.3, Target.W: 5.0, Target.R: 0.3, Target.T_TRIB: 0.3},
    }
    return table.get(reading.level, {}).get(target, 1.0)


def _f8_target_likelihood(reading: FilterReading, target: Target) -> float:
    """F8 Repair Topology - target component."""
    table = {
        ReadingLevel.STRONG_FOR: {Target.C: 5.0, Target.E: 0.1, Target.S: 1.0, Target.W: 0.5, Target.R: 0.5, Target.T_TRIB: 1.0},
        ReadingLevel.WEAK_FOR: {Target.C: 2.0, Target.E: 0.5, Target.S: 1.0, Target.W: 0.5, Target.R: 1.0, Target.T_TRIB: 4.0},
        ReadingLevel.NEUTRAL: {Target.C: 1.0, Target.E: 1.0, Target.S: 1.0, Target.W: 1.0, Target.R: 1.0, Target.T_TRIB: 1.0},
        ReadingLevel.WEAK_AGAINST: {Target.C: 0.3, Target.E: 4.0, Target.S: 0.5, Target.W: 1.0, Target.R: 1.0, Target.T_TRIB: 0.5},
        ReadingLevel.STRONG_AGAINST: {Target.C: 0.1, Target.E: 2.0, Target.S: 0.5, Target.W: 4.0, Target.R: 0.5, Target.T_TRIB: 0.3},
    }
    return table.get(reading.level, {}).get(target, 1.0)


def likelihood_joint(reading: FilterReading, arch: Architecture, target: Target) -> float:
    """P(reading | architecture, target).

    Per Eq 19, factored as P(reading | arch) * P(reading | target) under conditional
    independence assumption. For dual-purpose filters (F8, F12), the joint structure
    matters more; this implementation treats them as approximate factors.
    """
    return likelihood_architecture(reading, arch) * likelihood_target(reading, target)


# ============================================================================
# Helper functions
# ============================================================================

def _scale_by_reading_level(base_likelihood: float, level: ReadingLevel) -> float:
    """Scale a base "strong_for" likelihood by the reading level.

    "strong_for" returns base_likelihood unchanged.
    "weak_for" returns sqrt(base_likelihood) approximately (less extreme).
    "neutral" returns 1.0 (uninformative).
    "weak_against" returns 1/sqrt(base_likelihood).
    "strong_against" returns 1/base_likelihood.

    This implements the ordinal scaling so that the same filter at different reading
    levels produces likelihood ratios consistent with ORDINAL_LIKELIHOOD_RATIOS.
    """
    if level == ReadingLevel.STRONG_FOR:
        return base_likelihood
    elif level == ReadingLevel.WEAK_FOR:
        # Compress toward 1 (less extreme)
        if base_likelihood > 1:
            return 1 + (base_likelihood - 1) * 0.4
        else:
            return 1 - (1 - base_likelihood) * 0.4
    elif level == ReadingLevel.NEUTRAL:
        return 1.0
    elif level == ReadingLevel.WEAK_AGAINST:
        # Inverse direction, compressed
        inv = 1.0 / max(base_likelihood, 0.01)
        if inv > 1:
            return 1 + (inv - 1) * 0.4
        else:
            return 1 - (1 - inv) * 0.4
    elif level == ReadingLevel.STRONG_AGAINST:
        return 1.0 / max(base_likelihood, 0.01)
    return 1.0


def _f6_likelihood(reading: FilterReading, arch: Architecture) -> float:
    """F6 has categorical readings: depletion direction.

    Reading levels (custom for F6):
        STRONG_FOR    -> "instrumentalism intensifies" (TA signature)
        WEAK_FOR      -> "parentified posture drops" (TB signature)
        NEUTRAL       -> "no change" (A signature; substrate already at baseline)
        WEAK_AGAINST  -> "mask slips" (I signature)
        STRONG_AGAINST -> "somatic-cognitive split" (H signature)

    This non-standard mapping requires custom likelihood handling.
    Operators using F6 should ensure their reading level corresponds to the
    intended depletion direction.
    """
    f6_table = {
        ReadingLevel.STRONG_FOR: {
            # instrumentalism_intensifies - TA signature
            Architecture.A: 0.5, Architecture.TA: 5.0, Architecture.TB: 0.3,
            Architecture.TA_OD: 4.0, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 0.5, Architecture.I_BETA: 0.5,
            Architecture.H: 1.0, Architecture.D: 1.0,
        },
        ReadingLevel.WEAK_FOR: {
            # parentified_posture_drops - TB signature
            Architecture.A: 1.0, Architecture.TA: 0.3, Architecture.TB: 5.0,
            Architecture.TA_OD: 0.5, Architecture.TB_OD: 4.0,
            Architecture.I_ALPHA: 0.1, Architecture.I_BETA: 0.1,
            Architecture.H: 0.3, Architecture.D: 0.5,
        },
        ReadingLevel.NEUTRAL: {
            # no_change - A signature
            Architecture.A: 4.0, Architecture.TA: 1.0, Architecture.TB: 1.0,
            Architecture.TA_OD: 1.0, Architecture.TB_OD: 1.0,
            Architecture.I_ALPHA: 0.5, Architecture.I_BETA: 0.5,
            Architecture.H: 1.0, Architecture.D: 2.0,
        },
        ReadingLevel.WEAK_AGAINST: {
            # mask_slips - I signature
            Architecture.A: 0.05, Architecture.TA: 0.3, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.3, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 5.0, Architecture.I_BETA: 4.0,
            Architecture.H: 3.0, Architecture.D: 1.0,
        },
        ReadingLevel.STRONG_AGAINST: {
            # somatic_cognitive_split - H signature
            Architecture.A: 0.1, Architecture.TA: 0.3, Architecture.TB: 0.5,
            Architecture.TA_OD: 0.3, Architecture.TB_OD: 0.5,
            Architecture.I_ALPHA: 0.3, Architecture.I_BETA: 0.1,
            Architecture.H: 5.0, Architecture.D: 0.3,
        },
    }
    return f6_table.get(reading.level, {}).get(arch, 1.0)


# ============================================================================
# Filter type queries (for protocol logic)
# ============================================================================

def is_architecture_probe(filter_id: str) -> bool:
    return filter_id in ARCHITECTURE_FILTER_DIRECTIONS or filter_id in SUBSTRATE_PHYSICS_FILTER_DIRECTIONS

def is_target_probe(filter_id: str) -> bool:
    return filter_id in TARGET_FILTER_DIRECTIONS

def is_dual_purpose(filter_id: str) -> bool:
    return filter_id in DUAL_PURPOSE_FILTER_TARGET_DIRECTIONS

def is_substrate_physics(filter_id: str) -> bool:
    return filter_id in SUBSTRATE_PHYSICS_FILTER_DIRECTIONS

def is_target_trajectory(filter_id: str) -> bool:
    return filter_id in TARGET_TRAJECTORY_FILTERS

def requires_cross_dyad(filter_id: str) -> bool:
    return filter_id in {"F4", "F13b", "F15b"}

def requires_longitudinal(filter_id: str) -> bool:
    return filter_id in {"F11", "F17", "F13a", "F14a"}
