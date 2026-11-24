use super::*;

#[test]
fn general_aura_kernel_synthetic_capacity_chain_is_iteration_order_independent() {
    // SYNTHETIC dense scratch population at the four-seat physical-card
    // bound. This tests allocation/iteration, not paid reachability or tokens.
    let count = capacities::OWNED_LIMIT * 4;
    let effects: Vec<_> = (1..count)
        .map(|source| AuraControlEffect {
            source,
            target: source - 1,
            timestamp: source as u64,
        })
        .collect();
    let initial: Vec<_> = (0..count).map(|i| PlayerId((i % 4) as u8)).collect();
    let live = [PlayerId(0), PlayerId(1), PlayerId(2), PlayerId(3)];
    for order in [effects.clone(), effects.iter().rev().copied().collect()] {
        let mut controllers = initial.clone();
        apply_aura_control_effects(&order, &mut controllers, &live).unwrap();
        assert!(controllers.iter().all(|p| *p == initial[count - 1]));
    }
    // Empty population and independent grants remain bounded as well.
    apply_aura_control_effects(&[], &mut [], &live).unwrap();
}
