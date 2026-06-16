use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ts_collections::SyncSet;

use crate::{
    BreadthFirstSearchOptions, breadth_first_search_parallel, breadth_first_search_parallel_ex,
    identity,
};

#[test]
fn test_breadth_first_search_parallel_basic_functionality_find_specific_node() {
    // Test basic functionality with a simple DAG
    // Graph: A -> B, A -> C, B -> D, C -> D
    let graph = HashMap::from([
        ("A", vec!["B", "C"]),
        ("B", vec!["D"]),
        ("C", vec!["D"]),
        ("D", vec![]),
    ]);

    let result = breadth_first_search_parallel(
        "A",
        move |node| graph[node].clone(),
        |node| (node == "D", true),
    );
    assert_eq!(result.stopped, true, "Expected search to stop at D");
    assert_eq!(result.path, vec!["D", "B", "A"]);
}

#[test]
fn test_breadth_first_search_parallel_basic_functionality_visit_all_nodes() {
    // Test basic functionality with a simple DAG
    // Graph: A -> B, A -> C, B -> D, C -> D
    let graph = HashMap::from([
        ("A", vec!["B", "C"]),
        ("B", vec!["D"]),
        ("C", vec!["D"]),
        ("D", vec![]),
    ]);

    let visited_nodes = Arc::new(Mutex::new(Vec::new()));
    let visited_nodes_for_visit = visited_nodes.clone();
    let result = breadth_first_search_parallel(
        "A",
        move |node| graph[node].clone(),
        move |node| {
            visited_nodes_for_visit.lock().unwrap().push(node);
            (false, false) // Never stop early
        },
    );

    // Should return nil since we never return true
    assert_eq!(result.stopped, false, "Expected search to not stop early");
    assert!(
        result.path.is_empty(),
        "Expected nil path when visit function never returns true"
    );

    // Should visit all nodes exactly once
    let mut visited_nodes = Arc::try_unwrap(visited_nodes)
        .unwrap()
        .into_inner()
        .unwrap();
    visited_nodes.sort();
    let expected = vec!["A", "B", "C", "D"];
    assert_eq!(visited_nodes, expected);
}

#[test]
fn test_breadth_first_search_parallel_early_termination() {
    // Test that nodes below the target level are not visited
    let graph = HashMap::from([
        ("Root", vec!["L1A", "L1B"]),
        ("L1A", vec!["L2A", "L2B"]),
        ("L1B", vec!["L2C"]),
        ("L2A", vec!["L3A"]),
        ("L2B", vec![]),
        ("L2C", vec![]),
        ("L3A", vec![]),
    ]);

    let visited = SyncSet::new();
    breadth_first_search_parallel_ex(
        "Root",
        move |node| graph[node].clone(),
        |node| (node == "L2B", true), // Stop at level 2
        BreadthFirstSearchOptions {
            visited: Some(visited.clone()),
            ..Default::default()
        },
        identity,
    );

    assert!(visited.has(&"Root"), "Expected to visit Root");
    assert!(visited.has(&"L1A"), "Expected to visit L1A");
    assert!(visited.has(&"L1B"), "Expected to visit L1B");
    assert!(visited.has(&"L2A"), "Expected to visit L2A");
    assert!(visited.has(&"L2B"), "Expected to visit L2B");
    // L2C is non-deterministic
    assert!(!visited.has(&"L3A"), "Expected not to visit L3A");
}

#[test]
fn test_breadth_first_search_parallel_returns_fallback_when_no_other_result_found() {
    // Test that fallback behavior works correctly
    let graph = HashMap::from([
        ("A", vec!["B", "C"]),
        ("B", vec!["D"]),
        ("C", vec!["D"]),
        ("D", vec![]),
    ]);

    let visited = SyncSet::new();
    let result = breadth_first_search_parallel_ex(
        "A",
        move |node| graph[node].clone(),
        |node| (node == "A", false), // Record A as a fallback, but do not stop
        BreadthFirstSearchOptions {
            visited: Some(visited.clone()),
            ..Default::default()
        },
        identity,
    );

    assert_eq!(result.stopped, false, "Expected search to not stop early");
    assert_eq!(result.path, vec!["A"]);
    assert!(visited.has(&"B"), "Expected to visit B");
    assert!(visited.has(&"C"), "Expected to visit C");
    assert!(visited.has(&"D"), "Expected to visit D");
}

#[test]
fn test_breadth_first_search_parallel_returns_a_stop_result_over_a_fallback() {
    // Test that a stop result is preferred over a fallback
    let graph = HashMap::from([
        ("A", vec!["B", "C"]),
        ("B", vec!["D"]),
        ("C", vec!["D"]),
        ("D", vec![]),
    ]);

    let result = breadth_first_search_parallel(
        "A",
        move |node| graph[node].clone(),
        |node| match node {
            "A" => (true, false), // Record fallback
            "D" => (true, true),  // Stop at D
            _ => (false, false),
        },
    );

    assert_eq!(result.stopped, true, "Expected search to stop at D");
    assert_eq!(result.path, vec!["D", "B", "A"]);
}
