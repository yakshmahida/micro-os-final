//! Tier 1 & Tier 2 E2E Tests for R3: Dijkstra's Banker's Algorithm Deadlock Avoidance.
//! CS2001 OS Capstone | Student: Yakshendrasinh R Mahida (USN: 25BTRCL288)

use micro_os::deadlock::BankersAlgorithm;

// Helper to set up the classic 5-process 3-resource textbook Banker scenario
fn setup_standard_banker() -> BankersAlgorithm {
    let mut banker = BankersAlgorithm::new(5, 3);
    banker.set_available(&[3, 3, 2]);
    banker.set_allocation(0, &[0, 1, 0]);
    banker.set_max(0, &[7, 5, 3]);
    banker.set_allocation(1, &[2, 0, 0]);
    banker.set_max(1, &[3, 2, 2]);
    banker.set_allocation(2, &[3, 0, 2]);
    banker.set_max(2, &[9, 0, 2]);
    banker.set_allocation(3, &[2, 1, 1]);
    banker.set_max(3, &[2, 2, 2]);
    banker.set_allocation(4, &[0, 0, 2]);
    banker.set_max(4, &[4, 3, 3]);
    banker
}

// ============================================================================
// Tier 1: Feature Coverage (Happy Path)
// ============================================================================

/// `TC-T1-22`: Banker Safe State Detection.
/// Validates known safe state evaluates to `is_safe() == true`.
#[test]
fn t1_banker_22_banker_safe_state_detection() {
    let banker = setup_standard_banker();
    assert!(
        banker.is_safe(),
        "Standard textbook Banker state must evaluate to SAFE"
    );
}

/// `TC-T1-23`: Banker Safe Sequence Generation.
/// Validates `safe_sequence()` derives valid topological execution order.
#[test]
fn t1_banker_23_banker_safe_sequence_generation() {
    let banker = setup_standard_banker();
    let seq = banker.safe_sequence();
    assert!(seq.is_some(), "Safe sequence must exist for safe state");

    let sequence = seq.unwrap();
    assert_eq!(
        sequence.len(),
        5,
        "Safe sequence must include all 5 processes"
    );

    // Verify validity: each process's Need <= Available at execution step
    let mut avail = banker.available.clone();
    for &pid in &sequence {
        for r in 0..3 {
            assert!(
                banker.need(pid, r) <= avail[r],
                "Process {} Need[{}] ({}) must be <= Available[{}] ({})",
                pid,
                r,
                banker.need(pid, r),
                r,
                avail[r]
            );
            avail[r] += banker.allocation[pid][r];
        }
    }
}

/// `TC-T1-24`: Banker Safe Request Approval.
/// Validates safe resource request is granted and matrices update.
#[test]
fn t1_banker_24_banker_safe_request_approval() {
    let mut banker = setup_standard_banker();

    // P1 requests [1, 0, 2] (safe request)
    let granted = banker.request_resources(1, &[1, 0, 2]);
    assert!(granted, "Safe request by P1 must be approved");

    // Check updated matrices
    assert_eq!(
        banker.available,
        vec![2, 3, 0],
        "Available must be [2, 3, 0]"
    );
    assert_eq!(
        banker.allocation[1],
        vec![3, 0, 2],
        "P1 allocation must be [3, 0, 2]"
    );
    assert_eq!(banker.need(1, 0), 0);
    assert_eq!(banker.need(1, 1), 2);
    assert_eq!(banker.need(1, 2), 0);
    assert!(banker.is_safe(), "State after request must remain SAFE");
}

/// `TC-T1-25`: Banker Unsafe Request Rejection.
/// Validates deadlock-inducing request is rejected and state rolled back.
#[test]
fn t1_banker_25_banker_unsafe_request_rejection() {
    let mut banker = setup_standard_banker();
    let original_available = banker.available.clone();
    let original_allocation = banker.allocation.clone();
    let original_need = banker.get_need_matrix();

    // P0 requests [0, 3, 0] (leads to unsafe state by exhausting resource B)
    let granted = banker.request_resources(0, &[0, 3, 0]);
    assert!(!granted, "Unsafe request by P0 must be rejected");

    // State must be completely restored
    assert_eq!(
        banker.available, original_available,
        "Available must be restored after rollback"
    );
    assert_eq!(
        banker.allocation, original_allocation,
        "Allocation must be restored after rollback"
    );
    assert_eq!(
        banker.get_need_matrix(), original_need,
        "Need must be restored after rollback"
    );
}

/// `TC-T1-26`: Banker Resource Deallocation.
/// Validates terminated task releases allocations back to Available pool.
#[test]
fn t1_banker_26_banker_resource_deallocation() {
    let mut banker = setup_standard_banker();
    let initial_p2_alloc = banker.allocation[2].clone(); // [3, 0, 2]
    let initial_avail = banker.available.clone();         // [3, 3, 2]

    let released = banker.release_resources(2, &[3, 0, 2]);
    assert!(released, "Release of P2 resources must succeed");

    assert_eq!(
        banker.allocation[2],
        vec![0, 0, 0],
        "P2 allocation must be zero after release"
    );
    assert_eq!(
        banker.available,
        vec![
            initial_avail[0] + initial_p2_alloc[0],
            initial_avail[1] + initial_p2_alloc[1],
            initial_avail[2] + initial_p2_alloc[2],
        ],
        "Available must increase by released resources"
    );
}

// ============================================================================
// Tier 2: Boundary & Corner Cases (Stress & Limits)
// ============================================================================

/// `TC-T2-23`: Banker Request Exceeding Need.
/// Validates process requesting more than declared Need is rejected.
#[test]
fn t2_banker_23_banker_request_exceeding_need() {
    let mut banker = setup_standard_banker();
    // P1 Need is [1, 2, 2]. Request [2, 0, 0] exceeds Need[0] (2 > 1)
    let granted = banker.request_resources(1, &[2, 0, 0]);
    assert!(
        !granted,
        "Request exceeding declared Need must be rejected immediately"
    );
}

/// `TC-T2-24`: Banker Request Exceeding Available.
/// Validates process requesting more than system Available pool is rejected.
#[test]
fn t2_banker_24_banker_request_exceeding_available() {
    let mut banker = setup_standard_banker();
    // Available is [3, 3, 2]. P0 Need is [7, 4, 3]. Request [4, 0, 0] exceeds Available[0]
    let granted = banker.request_resources(0, &[4, 0, 0]);
    assert!(
        !granted,
        "Request exceeding Available pool must be rejected immediately"
    );
}

/// `TC-T2-25`: Circular Wait Deadlock Inducing Request.
/// Validates request eliminating all safe sequences is rejected.
#[test]
fn t2_banker_25_circular_wait_deadlock_inducing_request() {
    let mut banker = BankersAlgorithm::new(2, 2);
    banker.set_available(&[1, 1]);
    banker.set_allocation(0, &[1, 0]);
    banker.set_max(0, &[1, 1]);
    banker.set_allocation(1, &[0, 1]);
    banker.set_max(1, &[1, 1]);

    // Initial state is safe because Available=[1, 1] can satisfy either P0 or P1
    assert!(banker.is_safe());

    // If Available was [0, 0], they would deadlock
    let mut deadlocked = BankersAlgorithm::new(2, 2);
    deadlocked.set_available(&[0, 0]);
    deadlocked.set_allocation(0, &[1, 0]);
    deadlocked.set_max(0, &[1, 1]);
    deadlocked.set_allocation(1, &[0, 1]);
    deadlocked.set_max(1, &[1, 1]);

    assert!(
        !deadlocked.is_safe(),
        "State with circular wait potential and zero available must be UNSAFE"
    );
}

/// `TC-T2-26`: Zero Available Resources Matrix.
/// Validates system with Available all zeros and positive Need evaluates to unsafe.
#[test]
fn t2_banker_26_zero_available_resources_matrix() {
    let mut banker = BankersAlgorithm::new(3, 2);
    banker.set_available(&[0, 0]);
    banker.set_allocation(0, &[1, 1]);
    banker.set_max(0, &[2, 2]);
    banker.set_allocation(1, &[1, 0]);
    banker.set_max(1, &[2, 1]);
    banker.set_allocation(2, &[0, 1]);
    banker.set_max(2, &[1, 2]);

    assert!(
        !banker.is_safe(),
        "Zero available resources with non-zero Need must not be safe"
    );
    assert_eq!(
        banker.safe_sequence(),
        None,
        "No safe sequence can exist with zero available resources"
    );
}

/// `TC-T2-27`: Full Demand Allocation Termination.
/// Validates process allocation reaching maximum demand (Need == 0) releases cleanly.
#[test]
fn t2_banker_27_full_demand_allocation_termination() {
    let mut banker = BankersAlgorithm::new(2, 2);
    banker.set_available(&[2, 2]);
    banker.set_allocation(0, &[1, 1]);
    banker.set_max(0, &[2, 2]);
    banker.set_allocation(1, &[0, 0]);
    banker.set_max(1, &[1, 1]);

    // P0 requests its remaining need [1, 1]
    let granted = banker.request_resources(0, &[1, 1]);
    assert!(granted, "Full demand request must be granted");
    assert_eq!(banker.need(0, 0), 0);
    assert_eq!(banker.need(0, 1), 0);

    // P0 completes and releases all resources
    let released = banker.release_resources(0, &[2, 2]);
    assert!(released);
    assert_eq!(
        banker.available,
        vec![3, 3],
        "Available must reflect release of all P0 resources"
    );
}
