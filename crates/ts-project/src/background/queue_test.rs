use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI64, Ordering},
};

use super::new_queue;
use ts_core::Context;

#[test]
fn test_queue_basic_enqueue() {
    let q = new_queue();
    let executed = Arc::new(Mutex::new(false));
    let executed_clone = executed.clone();
    q.enqueue(Context::background(), move |_| {
        *executed_clone.lock().unwrap_or_else(|err| err.into_inner()) = true;
    });

    q.wait();

    assert!(*executed.lock().unwrap_or_else(|err| err.into_inner()));
    q.close();
}

#[test]
fn test_queue_multiple_tasks_execution() {
    let q = new_queue();
    let counter = Arc::new(AtomicI64::new(0));
    let num_tasks = 10;

    for _ in 0..num_tasks {
        let counter = counter.clone();
        q.enqueue(Context::background(), move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    q.wait();

    assert_eq!(counter.load(Ordering::SeqCst), num_tasks);
    q.close();
}

#[test]
fn test_queue_nested_enqueue() {
    let q = new_queue();
    let executed = Arc::new(Mutex::new(Vec::new()));

    let executed_parent = executed.clone();
    let q_child = q.clone();
    q.enqueue(Context::background(), move |ctx| {
        executed_parent
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .push("parent");

        let executed_child = executed_parent.clone();
        q_child.enqueue(ctx, move |_| {
            executed_child
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .push("child");
        });
    });

    q.wait();

    assert_eq!(
        executed.lock().unwrap_or_else(|err| err.into_inner()).len(),
        2
    );
    q.close();
}

#[test]
fn test_queue_closed_queue_rejects_new_tasks() {
    let q = new_queue();
    q.close();

    let executed = Arc::new(Mutex::new(false));
    let executed_clone = executed.clone();
    q.enqueue(Context::background(), move |_| {
        *executed_clone.lock().unwrap_or_else(|err| err.into_inner()) = true;
    });

    q.wait();

    assert!(
        !*executed.lock().unwrap_or_else(|err| err.into_inner()),
        "Task should not execute after queue is closed"
    );
}
