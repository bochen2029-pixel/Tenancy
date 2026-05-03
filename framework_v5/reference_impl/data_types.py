"""
Ground Truth Framework — Reference Implementation
Data Types

Core data structures for filter readings, classifications, and posterior distributions.

Reference: framework.md §3 (filters), §3.7 (Bayesian formulation), §2 (classification)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


# ============================================================================
# Architecture, Target, Stake — the classification triple
# ============================================================================

class Architecture(str, Enum):
    """The 9 architectures defined in framework.md §2.1.

    Time constant: years to decades. Approximately fixed within standard observation windows.
    """
    A = "A"                       # Autotelic-Coherent
    TA = "TA"                     # Trauma-Armored
    TB = "TB"                     # Boundary-Diffuse
    TA_OD = "TA-OD"               # Trauma-Armored with Ontological-Deception modifier
    TB_OD = "TB-OD"               # Boundary-Diffuse with Ontological-Deception modifier
    I_ALPHA = "I_alpha"           # Strategic Instrumental
    I_BETA = "I_beta"             # Banal Instrumental
    H = "H"                       # Ego-Syntonic Hybrid
    D = "D"                       # Developmental Absence


class Target(str, Enum):
    """The 6 targets defined in framework.md §2.2.

    Time constant: days to weeks. Per-dyad classification.
    Note: Targets are formally a probability distribution; categorical classification
    ships argmax. Mixed states (C+R, S+E) are real.
    """
    C = "C"               # Committed-Autotelic
    E = "E"               # Extractive
    S = "S"               # Suspended-Resolution (verification-mode)
    W = "W"               # Withdrawn (Honest-Instrumental)
    R = "R"               # Regulatory-Subcontracting
    T_TRIB = "T_trib"     # Tribute


@dataclass(frozen=True)
class Stake:
    """Stake in [0, 1]; property of the dyad.

    Operationalized per Eq (Sk):
        Sk(d) = alpha * arch_fraction(s, d) + (1 - alpha) * (1 - replaceability(d))

    Reference: framework.md §2.3
    """
    value: float
    arch_fraction: Optional[float] = None
    replaceability: Optional[float] = None
    alpha: float = 0.5

    def __post_init__(self):
        if not (0 <= self.value <= 1):
            raise ValueError(f"Stake value must be in [0, 1], got {self.value}")

    @classmethod
    def from_components(
        cls,
        arch_fraction: float,
        replaceability: float,
        alpha: float = 0.5,
    ) -> Stake:
        """Construct Stake from its operational components per Eq (Sk)."""
        if not (0 <= arch_fraction <= 1):
            raise ValueError(f"arch_fraction must be in [0, 1], got {arch_fraction}")
        if not (0 <= replaceability <= 1):
            raise ValueError(f"replaceability must be in [0, 1], got {replaceability}")
        if not (0 <= alpha <= 1):
            raise ValueError(f"alpha must be in [0, 1], got {alpha}")
        value = alpha * arch_fraction + (1 - alpha) * (1 - replaceability)
        return cls(
            value=value,
            arch_fraction=arch_fraction,
            replaceability=replaceability,
            alpha=alpha,
        )

    @property
    def category(self) -> str:
        """Categorical Stake level per thresholds.yaml."""
        if self.value > 0.7:
            return "high_stake"
        elif self.value >= 0.4:
            return "moderate_stake"
        else:
            return "low_stake"


# ============================================================================
# Filter readings
# ============================================================================

class ReadingLevel(str, Enum):
    """Ordinal scale for filter readings.

    Default likelihood ratios:
        strong_for:    5x
        weak_for:      2x
        neutral:       1x
        weak_against:  0.5x
        strong_against: 0.2x

    Reference: framework.md §3.7, spec/thresholds.yaml
    """
    STRONG_FOR = "strong_for"
    WEAK_FOR = "weak_for"
    NEUTRAL = "neutral"
    WEAK_AGAINST = "weak_against"
    STRONG_AGAINST = "strong_against"


@dataclass
class FilterReading:
    """A single filter reading from observation.

    Attributes:
        filter_id: Filter identifier (e.g., "F1", "F2", "SP1")
        level: Ordinal reading level
        timestamp: Observation time (in months from observation start)
        contaminated: If True, observation was analytically contaminated
            (per Protocol B); excluded from evidence base
        notes: Free-text annotation for observation context
    """
    filter_id: str
    level: ReadingLevel
    timestamp: float = 0.0
    contaminated: bool = False
    notes: str = ""

    def is_admissible(self) -> bool:
        """Per Protocol B (Eq 18), contaminated readings are excluded from evidence."""
        return not self.contaminated


@dataclass
class ObservationWindow:
    """A full observation window with all filter readings.

    Attributes:
        readings: All filter readings collected
        duration_months: Total observation duration
        cross_dyad_breadth: Number of dyads observed (for Protocol F)
        observer_joy_capability_rate: Fraction of observations where observer was
            joy-capable (per Protocol B). Below threshold = framework outputs contaminated.
    """
    readings: list[FilterReading] = field(default_factory=list)
    duration_months: float = 0.0
    cross_dyad_breadth: int = 1
    observer_joy_capability_rate: float = 1.0

    def admissible_readings(self) -> list[FilterReading]:
        """Return only Protocol B-admissible readings."""
        return [r for r in self.readings if r.is_admissible()]

    def filter_readings(self, filter_id: str) -> list[FilterReading]:
        """Return all admissible readings for a specific filter."""
        return [r for r in self.admissible_readings() if r.filter_id == filter_id]

    def is_protocol_C_compliant(self) -> bool:
        """Check if observation window respects Protocol C bounds."""
        return self.duration_months <= 36  # max for F11 longitudinal extension

    def is_protocol_F_compliant(self) -> bool:
        """Check if observation breadth allows Protocol F (cross-dyad)."""
        return self.cross_dyad_breadth >= 3

    def is_observer_audit_passing(self) -> bool:
        """Per Protocol B, observer must be joy-capable in >=30% of observations."""
        return self.observer_joy_capability_rate >= 0.3


# ============================================================================
# Posterior distribution over the classification triple
# ============================================================================

@dataclass
class Classification:
    """A categorical classification output.

    Attributes:
        architecture: Inferred architecture
        target: Inferred target (per dyad)
        stake: Inferred stake
        bayes_factor: P(this classification) / P(second-most-likely classification)
        confidence_level: "preliminary" | "standard" | "high_confidence" | "insufficient"
        posterior_distribution: Full posterior over all (A, T, Sk) triples
        notes: Diagnostic notes including any failure modes detected
    """
    architecture: Architecture
    target: Target
    stake: Stake
    bayes_factor: float
    confidence_level: str
    posterior_distribution: dict
    notes: list[str] = field(default_factory=list)

    @property
    def is_shippable(self) -> bool:
        """Whether this classification meets at least preliminary confidence threshold."""
        return self.confidence_level != "insufficient"

    def __str__(self) -> str:
        return (
            f"Classification("
            f"arch={self.architecture.value}, "
            f"target={self.target.value}, "
            f"stake={self.stake.category}, "
            f"BF={self.bayes_factor:.1f}, "
            f"confidence={self.confidence_level})"
        )


@dataclass
class FailureModeDetection:
    """Detected failure modes per docs/failure_modes.md."""
    code: str
    description: str
    operational_response: str

    def __str__(self) -> str:
        return f"[{self.code}] {self.description} -> {self.operational_response}"


# ============================================================================
# Closed-loop dynamics state
# ============================================================================

@dataclass
class ClosedLoopState:
    """State of the §4.7 closed-loop dynamics.

    q is the observer's posterior probability of S->C trajectory.
    g(q) is access-grant function output.
    r(g) is subject's response to access level.

    Reference: framework.md §4.7, Eq 21
    """
    q: float                    # posterior probability in [0, 1]
    g: float                    # current access-grant level in [0, 1]
    last_observation: float     # subject's last observed response
    eta: float = 0.1            # update rate
    history: list[float] = field(default_factory=list)

    def is_at_committed_fixed_point(self, tolerance: float = 0.05) -> bool:
        return self.q > 1 - tolerance

    def is_at_rejected_fixed_point(self, tolerance: float = 0.05) -> bool:
        return self.q < tolerance

    def is_oscillating(self) -> bool:
        """Detect oscillation per thresholds.yaml: |dq| > 0.3 sustained over 3+ updates."""
        if len(self.history) < 3:
            return False
        recent = self.history[-3:]
        diffs = [abs(recent[i+1] - recent[i]) for i in range(len(recent)-1)]
        return all(d > 0.3 for d in diffs)
