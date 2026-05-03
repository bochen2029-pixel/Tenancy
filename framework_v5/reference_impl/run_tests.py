"""Integration test: run all 6 test cases against the classifier."""
from data_types import Architecture, Target, Stake, ReadingLevel, FilterReading, ObservationWindow
from classifier import GTPClassifier


def run_case(label, expected_arch, expected_target, readings, duration, breadth, joy_rate):
    obs = ObservationWindow(
        readings=readings,
        duration_months=duration,
        cross_dyad_breadth=breadth,
        observer_joy_capability_rate=joy_rate,
    )
    classifier = GTPClassifier()
    result = classifier.classify(obs)
    expected = f"{expected_arch} x {expected_target}"
    actual = f"{result.architecture.value} x {result.target.value}"

    arch_match = result.architecture.value == expected_arch
    target_match = result.target.value == expected_target

    if arch_match and target_match:
        status = "PASS"
    elif arch_match or target_match:
        status = "PARTIAL"
    else:
        status = "FAIL"

    print(f"[{status}] {label}")
    print(f"  Expected: {expected}")
    print(f"  Actual:   {actual}  (BF={result.bayes_factor:.1f}, conf={result.confidence_level})")
    sorted_cells = sorted(result.posterior_distribution.items(), key=lambda kv: kv[1], reverse=True)
    print(f"  Top 3:    {[(c, round(p, 3)) for c, p in sorted_cells[:3]]}")
    print()
    return status


results = []

# Case 01: A x C
results.append(run_case("Case 01: A x C", "A", "C", [
    FilterReading("F1", ReadingLevel.WEAK_AGAINST, 4.0),
    FilterReading("F2", ReadingLevel.STRONG_FOR, 5.0),
    FilterReading("F3", ReadingLevel.STRONG_FOR, 7.0),
    FilterReading("F4", ReadingLevel.WEAK_AGAINST, 10.0),
    FilterReading("F5", ReadingLevel.STRONG_FOR, 3.0),
    FilterReading("F6", ReadingLevel.NEUTRAL, 8.0),
    FilterReading("F7", ReadingLevel.STRONG_FOR, 14.0),
    FilterReading("F8", ReadingLevel.STRONG_FOR, 6.0),
    FilterReading("F9", ReadingLevel.STRONG_FOR, 11.0),
    FilterReading("F10", ReadingLevel.STRONG_FOR, 4.5),
    FilterReading("F11", ReadingLevel.WEAK_FOR, 18.0),
    FilterReading("F12", ReadingLevel.STRONG_FOR, 13.0),
    FilterReading("SP1", ReadingLevel.STRONG_FOR, 6.5),
    FilterReading("SP2", ReadingLevel.STRONG_FOR, 9.5),
    FilterReading("SP3", ReadingLevel.STRONG_FOR, 7.5),
], 18.0, 4, 0.85))

# Case 02: TA x C
results.append(run_case("Case 02: TA x C", "TA", "C", [
    FilterReading("F1", ReadingLevel.WEAK_AGAINST, 6.0),
    FilterReading("F2", ReadingLevel.STRONG_FOR, 20.0),
    FilterReading("F3", ReadingLevel.STRONG_FOR, 22.0),
    FilterReading("F4", ReadingLevel.WEAK_AGAINST, 12.0),
    FilterReading("F5", ReadingLevel.STRONG_FOR, 8.0),
    FilterReading("F6", ReadingLevel.STRONG_FOR, 14.0),
    FilterReading("F7", ReadingLevel.STRONG_FOR, 21.0),
    FilterReading("F8", ReadingLevel.STRONG_FOR, 11.0),
    FilterReading("F9", ReadingLevel.STRONG_FOR, 23.0),
    FilterReading("F10", ReadingLevel.STRONG_FOR, 22.0),
    FilterReading("F11", ReadingLevel.STRONG_FOR, 24.0),
    FilterReading("F12", ReadingLevel.STRONG_FOR, 18.0),
    FilterReading("F17", ReadingLevel.WEAK_FOR, 22.0),
    FilterReading("SP1", ReadingLevel.STRONG_FOR, 16.0),
    FilterReading("SP2", ReadingLevel.STRONG_FOR, 17.0),
    FilterReading("SP3", ReadingLevel.STRONG_FOR, 15.5),
], 24.0, 3, 0.75))

# Case 03: TB-OD x S
results.append(run_case("Case 03: TB-OD x S", "TB-OD", "S", [
    FilterReading("F1", ReadingLevel.NEUTRAL, 4.0),
    FilterReading("F2", ReadingLevel.STRONG_FOR, 6.0),
    FilterReading("F3", ReadingLevel.WEAK_FOR, 7.0),
    FilterReading("F4", ReadingLevel.WEAK_AGAINST, 9.0),
    FilterReading("F5", ReadingLevel.STRONG_FOR, 5.0),
    FilterReading("F6", ReadingLevel.WEAK_FOR, 10.0),
    FilterReading("F7", ReadingLevel.STRONG_FOR, 8.0),
    FilterReading("F8", ReadingLevel.STRONG_FOR, 6.5),
    FilterReading("F9", ReadingLevel.STRONG_FOR, 12.0),
    FilterReading("F10", ReadingLevel.STRONG_FOR, 11.5),
    FilterReading("F11", ReadingLevel.WEAK_FOR, 14.0),
    FilterReading("F12", ReadingLevel.WEAK_FOR, 5.5),
    FilterReading("F13a", ReadingLevel.STRONG_FOR, 13.0),
    FilterReading("F14a", ReadingLevel.STRONG_FOR, 12.0),
    FilterReading("F14b", ReadingLevel.STRONG_FOR, 9.5),
    FilterReading("F15a", ReadingLevel.STRONG_FOR, 11.0),
    FilterReading("F16", ReadingLevel.STRONG_FOR, 13.5),
    FilterReading("F17", ReadingLevel.WEAK_FOR, 13.0),
    FilterReading("SP1", ReadingLevel.STRONG_FOR, 7.5),
    FilterReading("SP2", ReadingLevel.STRONG_FOR, 10.5),
    FilterReading("SP3", ReadingLevel.STRONG_FOR, 8.5),
], 14.0, 5, 0.7))

# Case 04: I_alpha x E
results.append(run_case("Case 04: I_alpha x E", "I_alpha", "E", [
    FilterReading("F1", ReadingLevel.STRONG_FOR, 1.5),
    FilterReading("F2", ReadingLevel.STRONG_AGAINST, 2.0),
    FilterReading("F3", ReadingLevel.WEAK_AGAINST, 4.5),
    FilterReading("F4", ReadingLevel.STRONG_FOR, 3.0),
    FilterReading("F5", ReadingLevel.WEAK_FOR, 2.5),
    FilterReading("F6", ReadingLevel.WEAK_AGAINST, 6.0),
    FilterReading("F7", ReadingLevel.WEAK_AGAINST, 5.0),
    FilterReading("F8", ReadingLevel.STRONG_AGAINST, 6.5),
    FilterReading("F9", ReadingLevel.WEAK_AGAINST, 7.0),
    FilterReading("F10", ReadingLevel.STRONG_FOR, 4.0),
    FilterReading("F11", ReadingLevel.NEUTRAL, 9.0),
    FilterReading("F12", ReadingLevel.NEUTRAL, 8.0),
    FilterReading("SP1", ReadingLevel.STRONG_AGAINST, 4.7),
    FilterReading("SP2", ReadingLevel.STRONG_AGAINST, 4.7),
    FilterReading("SP3", ReadingLevel.STRONG_AGAINST, 5.5),
], 9.0, 3, 0.65))

# Case 05: H x E
results.append(run_case("Case 05: H x E", "H", "E", [
    FilterReading("F1", ReadingLevel.WEAK_FOR, 3.0),
    FilterReading("F2", ReadingLevel.WEAK_FOR, 5.0),
    FilterReading("F3", ReadingLevel.WEAK_AGAINST, 4.5),
    FilterReading("F4", ReadingLevel.WEAK_FOR, 6.0),
    FilterReading("F5", ReadingLevel.WEAK_AGAINST, 4.0),
    FilterReading("F6", ReadingLevel.STRONG_AGAINST, 8.0),
    FilterReading("F7", ReadingLevel.WEAK_AGAINST, 7.0),
    FilterReading("F8", ReadingLevel.WEAK_AGAINST, 8.5),
    FilterReading("F9", ReadingLevel.WEAK_AGAINST, 9.0),
    FilterReading("F10", ReadingLevel.WEAK_AGAINST, 5.5),
    FilterReading("F11", ReadingLevel.NEUTRAL, 12.0),
    FilterReading("F12", ReadingLevel.WEAK_AGAINST, 10.0),
    FilterReading("SP1", ReadingLevel.NEUTRAL, 7.5),
    FilterReading("SP2", ReadingLevel.STRONG_FOR, 7.7),
    FilterReading("SP3", ReadingLevel.WEAK_FOR, 8.0),
], 12.0, 4, 0.6))

# Case 06: D x W
results.append(run_case("Case 06: D x W", "D", "W", [
    FilterReading("F1", ReadingLevel.WEAK_FOR, 4.0),
    FilterReading("F2", ReadingLevel.NEUTRAL, 5.5),
    FilterReading("F3", ReadingLevel.WEAK_AGAINST, 6.0),
    FilterReading("F4", ReadingLevel.NEUTRAL, 7.0),
    FilterReading("F5", ReadingLevel.WEAK_FOR, 4.5),
    FilterReading("F6", ReadingLevel.NEUTRAL, 8.0),
    FilterReading("F7", ReadingLevel.WEAK_AGAINST, 7.5),
    FilterReading("F8", ReadingLevel.STRONG_AGAINST, 9.0),
    FilterReading("F9", ReadingLevel.STRONG_AGAINST, 8.5),
    FilterReading("F10", ReadingLevel.WEAK_AGAINST, 6.5),
    FilterReading("F11", ReadingLevel.WEAK_AGAINST, 12.0),
    FilterReading("F12", ReadingLevel.STRONG_AGAINST, 10.0),
    FilterReading("SP1", ReadingLevel.NEUTRAL, 8.7),
    FilterReading("SP2", ReadingLevel.STRONG_AGAINST, 8.7),
    FilterReading("SP3", ReadingLevel.WEAK_AGAINST, 9.5),
], 12.0, 3, 0.7))

print("=== SUMMARY ===")
print(f"Pass:    {results.count('PASS')} / 6")
print(f"Partial: {results.count('PARTIAL')} / 6")
print(f"Fail:    {results.count('FAIL')} / 6")
