"""
Ground Truth Framework — Reference Implementation
Closed-Loop Dynamics

Simulates the §4.7 closed-loop reflexivity dynamics per Eq (21).
The framework's classification IS the perturbation event; observer's response
to classification feeds back into the next classification update.

Reference: framework.md §4.7
"""

from __future__ import annotations
import math
from dataclasses import dataclass, field
from typing import Callable, Optional

from data_types import ClosedLoopState


# ============================================================================
# Standard access-grant function
# ============================================================================

def standard_access_grant(q: float, threshold_for_high_stake: float = 0.7) -> float:
    """g(q): how much access does observer extend at posterior q?

    Default: smooth sigmoid that ramps up between q=0.3 and q=0.7,
    then continues to increase to full at q=1.0. Below 0.3, very low access.

    Per thresholds.yaml: minimum_q_for_high_stake_access = 0.7.

    Reference: framework.md §4.7
    """
    if q < 0.3:
        return 0.05 * q / 0.3  # very minimal access
    if q < threshold_for_high_stake:
        # Ramp from 0.05 at q=0.3 to 0.5 at q=0.7
        t = (q - 0.3) / (threshold_for_high_stake - 0.3)
        return 0.05 + 0.45 * t
    # Above high-stake threshold, ramp to 1.0
    t = (q - threshold_for_high_stake) / (1.0 - threshold_for_high_stake)
    return 0.5 + 0.5 * t


# ============================================================================
# Subject response models (the "ground truth" of the simulation)
# ============================================================================

def genuine_s_mode_response(g: float, noise: float = 0.05) -> float:
    """Subject in genuine S-mode: response approaches 1 (commitment-coherent) as g -> 1.

    Models the case where subject was verifying autotelic safety; granting access
    relieves verification load and produces commitment-coherent response.
    """
    import random
    base = g  # response tracks access-grant
    return max(0.0, min(1.0, base + random.gauss(0, noise)))


def e_mode_with_cover_response(g: float, noise: float = 0.05) -> float:
    """Subject in E-mode-with-S-cover: response approaches 0 (extraction) as g -> 1.

    Models the case where subject was hoarding option-value via S-cover;
    granting access destroys option-value and produces extraction or rage.
    """
    import random
    if g > 0.7:
        # Cover blown; extraction or withdrawal
        base = max(0.0, 0.3 - g)  # falls off rapidly
    else:
        # Cover holding; appears responsive
        base = g * 0.8  # slightly less than genuine, but similar
    return max(0.0, min(1.0, base + random.gauss(0, noise)))


def mixed_target_response(g: float, c_weight: float = 0.5, noise: float = 0.05) -> float:
    """Subject in mixed C+S or S+E state."""
    import random
    base = c_weight * g + (1 - c_weight) * (g * 0.6)
    return max(0.0, min(1.0, base + random.gauss(0, noise)))


# ============================================================================
# Closed-loop simulation
# ============================================================================

@dataclass
class SimulationResult:
    """Output of closed-loop simulation."""
    final_q: float
    converged: bool
    converged_to_committed: bool
    converged_to_rejected: bool
    oscillating: bool
    history: list[float]
    iterations: int


def simulate_closed_loop(
    initial_q: float,
    subject_response_fn: Callable[[float], float],
    access_grant_fn: Callable[[float], float] = standard_access_grant,
    eta: float = 0.1,
    max_iterations: int = 200,
    convergence_tolerance: float = 0.02,
) -> SimulationResult:
    """Simulate the closed-loop dynamics per Eq (21).

    q(t + dt) = q(t) + eta * [observation(r(g(q(t)))) - prediction(q(t))]

    where prediction(q) = g(q) (under "if classification correct, response should match access").

    Args:
        initial_q: Starting posterior probability of S->C trajectory
        subject_response_fn: Function r(g) modeling subject's response
        access_grant_fn: Function g(q) determining observer's access-grant
        eta: Update rate
        max_iterations: Maximum simulation steps
        convergence_tolerance: |dq| below this = converged

    Returns:
        SimulationResult with trajectory and convergence info
    """
    state = ClosedLoopState(
        q=initial_q,
        g=access_grant_fn(initial_q),
        last_observation=0.0,
        eta=eta,
        history=[initial_q],
    )

    for iteration in range(max_iterations):
        # Compute access grant
        g = access_grant_fn(state.q)
        state.g = g

        # Subject responds
        observation = subject_response_fn(g)
        state.last_observation = observation

        # Prediction under "classification correct" hypothesis
        prediction = g  # if S->C, response should match access

        # Update posterior per Eq (21)
        delta = state.eta * (observation - prediction)
        new_q = max(0.0, min(1.0, state.q + delta))

        state.q = new_q
        state.history.append(new_q)

        # Check convergence
        if iteration > 5 and abs(delta) < convergence_tolerance:
            return SimulationResult(
                final_q=state.q,
                converged=True,
                converged_to_committed=state.is_at_committed_fixed_point(),
                converged_to_rejected=state.is_at_rejected_fixed_point(),
                oscillating=False,
                history=state.history,
                iterations=iteration + 1,
            )

        # Check oscillation
        if state.is_oscillating():
            return SimulationResult(
                final_q=state.q,
                converged=False,
                converged_to_committed=False,
                converged_to_rejected=False,
                oscillating=True,
                history=state.history,
                iterations=iteration + 1,
            )

    # Did not converge within max iterations
    return SimulationResult(
        final_q=state.q,
        converged=False,
        converged_to_committed=state.is_at_committed_fixed_point(),
        converged_to_rejected=state.is_at_rejected_fixed_point(),
        oscillating=False,
        history=state.history,
        iterations=max_iterations,
    )


# ============================================================================
# Failure mode detection in closed-loop dynamics
# ============================================================================

def detect_premature_access_grant(
    history: list[float],
    access_grant_fn: Callable[[float], float] = standard_access_grant,
    high_stake_threshold: float = 0.7,
) -> bool:
    """Detect premature access-grant: g(q) jumps to high access at low q.

    Per §4.7 failure mode: do not grant high-Stake access at posterior < 0.7.
    """
    for q in history:
        if q < high_stake_threshold and access_grant_fn(q) > 0.5:
            return True
    return False


def detect_update_rate_too_low(
    history: list[float],
    eta: float,
    expected_convergence_steps: int = 50,
) -> bool:
    """Detect update rate too low: dynamics not converging within expected time.

    Per §4.7 failure mode: too-low eta allows CDS subsidization to fund continued cover.
    """
    if len(history) < expected_convergence_steps:
        return False
    recent_change = abs(history[-1] - history[-expected_convergence_steps])
    return recent_change < 0.1 and eta < 0.05


def detect_update_rate_too_high(history: list[float]) -> bool:
    """Detect update rate too high: oscillation per ClosedLoopState.is_oscillating()."""
    if len(history) < 4:
        return False
    recent = history[-4:]
    diffs = [abs(recent[i+1] - recent[i]) for i in range(len(recent)-1)]
    return all(d > 0.3 for d in diffs)
