"""
Ground Truth Framework — Reference Implementation
Bayesian Classifier

Computes posterior over (Architecture, Target, Stake) given filter readings.
Implements Eq (19) and Eq (20) from framework.md.

Reference: framework.md §3.7
"""

from __future__ import annotations
import math
from typing import Optional

from data_types import (
    Architecture, Target, Stake, ReadingLevel, FilterReading,
    ObservationWindow, Classification, FailureModeDetection,
)
from filters import (
    likelihood_architecture, likelihood_target, likelihood_joint,
    is_architecture_probe, is_target_probe, requires_cross_dyad, requires_longitudinal,
)


# ============================================================================
# Accessibility map (from spec/accessibility_map.yaml)
# ============================================================================

# Boolean accessibility: True = accessible, False = forbidden
# 'c' = conditional (treated as 0.3 prior weight to reflect lower base rate)
ACCESSIBILITY_MAP: dict[Architecture, dict[Target, float]] = {
    Architecture.A: {
        Target.C: 1.0, Target.E: 0.0, Target.S: 0.3, Target.W: 1.0,
        Target.R: 0.0, Target.T_TRIB: 0.0,
    },
    Architecture.TA: {
        Target.C: 1.0, Target.E: 0.0, Target.S: 0.3, Target.W: 1.0,
        Target.R: 0.0, Target.T_TRIB: 0.3,
    },
    Architecture.TB: {
        Target.C: 1.0, Target.E: 1.0, Target.S: 1.0, Target.W: 1.0,
        Target.R: 1.0, Target.T_TRIB: 1.0,
    },
    Architecture.TA_OD: {
        Target.C: 0.3, Target.E: 0.3, Target.S: 1.0, Target.W: 1.0,
        Target.R: 1.0, Target.T_TRIB: 0.3,
    },
    Architecture.TB_OD: {
        Target.C: 0.3, Target.E: 0.3, Target.S: 1.0, Target.W: 1.0,
        Target.R: 1.0, Target.T_TRIB: 1.0,
    },
    Architecture.I_ALPHA: {
        # I_alpha can SIMULATE any target via PFC, but operative is E.
        # We give simulated targets very low prior to capture this.
        Target.C: 0.05, Target.E: 1.0, Target.S: 0.05, Target.W: 0.05,
        Target.R: 0.05, Target.T_TRIB: 0.05,
    },
    Architecture.I_BETA: {
        Target.C: 0.0, Target.E: 1.0, Target.S: 0.0, Target.W: 0.0,
        Target.R: 0.0, Target.T_TRIB: 0.0,
    },
    Architecture.H: {
        Target.C: 0.0, Target.E: 1.0, Target.S: 0.0, Target.W: 0.0,
        Target.R: 0.0, Target.T_TRIB: 0.0,
    },
    Architecture.D: {
        Target.C: 0.0, Target.E: 1.0, Target.S: 0.0, Target.W: 1.0,
        Target.R: 0.0, Target.T_TRIB: 0.0,
    },
}


# ============================================================================
# Bayesian classifier
# ============================================================================

class GTPClassifier:
    """Ground Truth Framework Bayesian classifier.

    Implements:
    - Eq (19): P(A,T,Sk | F_1,...,F_K) ∝ Π_k P(F_k | A,T,Sk) * P(A,T,Sk)
    - Eq (20): A_hat = argmax P(A | data); ship iff bayes_factor > rho_threshold

    Uses ordinal-default likelihoods from filters.py.
    """

    def __init__(
        self,
        rho_preliminary: float = 3.0,
        rho_standard: float = 10.0,
        rho_high_confidence: float = 30.0,
    ):
        self.rho_preliminary = rho_preliminary
        self.rho_standard = rho_standard
        self.rho_high_confidence = rho_high_confidence

    def classify(
        self,
        observation: ObservationWindow,
        stake: Optional[Stake] = None,
    ) -> Classification:
        """Run the Bayesian classifier on an observation window.

        Args:
            observation: ObservationWindow with filter readings
            stake: Optional Stake; if None, uniform over Stake categories

        Returns:
            Classification with full posterior distribution and decision
        """
        # Default stake if not provided
        if stake is None:
            stake = Stake(value=0.5)

        # Protocol B: filter contaminated readings
        admissible = observation.admissible_readings()

        # Detect failure modes
        failure_modes = self._detect_failure_modes(observation)

        if not admissible:
            return Classification(
                architecture=Architecture.A,  # placeholder
                target=Target.C,                # placeholder
                stake=stake,
                bayes_factor=0.0,
                confidence_level="insufficient",
                posterior_distribution={},
                notes=["No admissible readings"] + [str(fm) for fm in failure_modes],
            )

        # Compute posterior over (A, T) using Eq (19)
        posterior = self._compute_posterior(admissible, stake)

        # Apply decision rule per Eq (20)
        sorted_cells = sorted(
            posterior.items(),
            key=lambda kv: kv[1],
            reverse=True,
        )
        best_cell, best_prob = sorted_cells[0]
        second_cell, second_prob = sorted_cells[1] if len(sorted_cells) > 1 else (None, 1e-9)

        bayes_factor = best_prob / max(second_prob, 1e-9)
        confidence = self._determine_confidence(bayes_factor)

        best_arch, best_target = best_cell

        notes = [str(fm) for fm in failure_modes]
        if requires_cross_dyad_evidence_missing := self._cross_dyad_evidence_missing(observation):
            notes.append("Cross-dyad evidence missing; classification bounded by Protocol F constraint")

        return Classification(
            architecture=best_arch,
            target=best_target,
            stake=stake,
            bayes_factor=bayes_factor,
            confidence_level=confidence,
            posterior_distribution={
                f"{a.value} x {t.value}": p for (a, t), p in posterior.items()
            },
            notes=notes,
        )

    def architecture_marginal(
        self, observation: ObservationWindow
    ) -> dict[Architecture, float]:
        """Compute marginal posterior over Architecture only."""
        admissible = observation.admissible_readings()
        joint = self._compute_posterior(admissible, Stake(0.5))
        marginal: dict[Architecture, float] = {a: 0.0 for a in Architecture}
        for (a, t), p in joint.items():
            marginal[a] += p
        return marginal

    def target_marginal(
        self, observation: ObservationWindow
    ) -> dict[Target, float]:
        """Compute marginal posterior over Target only."""
        admissible = observation.admissible_readings()
        joint = self._compute_posterior(admissible, Stake(0.5))
        marginal: dict[Target, float] = {t: 0.0 for t in Target}
        for (a, t), p in joint.items():
            marginal[t] += p
        return marginal

    # ========================================================================
    # Internal: posterior computation
    # ========================================================================

    def _compute_posterior(
        self,
        readings: list[FilterReading],
        stake: Stake,
    ) -> dict[tuple[Architecture, Target], float]:
        """Eq (19): P(A,T | data) ∝ Π_k P(F_k | A,T) * P(A,T)."""
        # Working in log space to avoid underflow
        log_unnorm: dict[tuple[Architecture, Target], float] = {}

        for arch in Architecture:
            for target in Target:
                # Prior from accessibility map
                prior = ACCESSIBILITY_MAP[arch][target]
                if prior <= 0:
                    continue  # zero-prior cells excluded from posterior

                # Log prior + sum of log likelihoods
                log_p = math.log(prior)
                for reading in readings:
                    likelihood = likelihood_joint(reading, arch, target)
                    if likelihood > 0:
                        log_p += math.log(likelihood)
                    else:
                        log_p = -math.inf
                        break

                if log_p > -math.inf:
                    log_unnorm[(arch, target)] = log_p

        if not log_unnorm:
            return {}

        # Normalize via log-sum-exp
        max_log = max(log_unnorm.values())
        exp_normalized = {
            cell: math.exp(lp - max_log) for cell, lp in log_unnorm.items()
        }
        total = sum(exp_normalized.values())
        if total <= 0:
            return {}

        return {cell: p / total for cell, p in exp_normalized.items()}

    def _determine_confidence(self, bayes_factor: float) -> str:
        if bayes_factor >= self.rho_high_confidence:
            return "high_confidence"
        elif bayes_factor >= self.rho_standard:
            return "standard"
        elif bayes_factor >= self.rho_preliminary:
            return "preliminary"
        else:
            return "insufficient"

    def _detect_failure_modes(
        self, observation: ObservationWindow
    ) -> list[FailureModeDetection]:
        """Detect known failure modes per docs/failure_modes.md."""
        failures = []

        if not observation.is_observer_audit_passing():
            failures.append(FailureModeDetection(
                code="FM-OBS-01",
                description=f"Observer joy-capability rate {observation.observer_joy_capability_rate:.1%} below 30% threshold",
                operational_response="Outputs contaminated; reduce confidence reporting; consider observer self-audit per Protocol B",
            ))

        if not observation.is_protocol_C_compliant():
            failures.append(FailureModeDetection(
                code="FM-PRC-C-01",
                description=f"Observation duration {observation.duration_months}mo exceeds Protocol C limit of 36mo",
                operational_response="Decommission matrix; framework operating outside intended window",
            ))

        admissible = observation.admissible_readings()
        if len(admissible) < 5:
            failures.append(FailureModeDetection(
                code="FM-DAT-01",
                description=f"Only {len(admissible)} admissible readings; below minimum for stable classification",
                operational_response="Extend observation; collect at least 5-10 admissible readings before classifying",
            ))

        return failures

    def _cross_dyad_evidence_missing(self, observation: ObservationWindow) -> bool:
        """Check if classification would benefit from cross-dyad evidence not available."""
        if observation.is_protocol_F_compliant():
            return False
        # Check if any admissible reading uses a cross-dyad filter
        cross_dyad_readings = [
            r for r in observation.admissible_readings()
            if requires_cross_dyad(r.filter_id)
        ]
        # If we have cross-dyad readings but no breadth, flag
        if cross_dyad_readings and observation.cross_dyad_breadth < 3:
            return True
        return False


# ============================================================================
# Convenience entry point
# ============================================================================

def classify_observation(
    readings: list[FilterReading],
    duration_months: float = 9.0,
    cross_dyad_breadth: int = 1,
    observer_joy_capability_rate: float = 1.0,
    stake: Optional[Stake] = None,
) -> Classification:
    """One-shot classification for simple cases."""
    obs = ObservationWindow(
        readings=readings,
        duration_months=duration_months,
        cross_dyad_breadth=cross_dyad_breadth,
        observer_joy_capability_rate=observer_joy_capability_rate,
    )
    classifier = GTPClassifier()
    return classifier.classify(obs, stake)
