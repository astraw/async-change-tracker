use std::{cell::RefCell, rc::Rc, sync::Arc};

use futures::stream::StreamExt;
use parking_lot::Mutex;

use async_change_tracker::ChangeTracker;

#[test]
fn test_change_tracker() {
    #[derive(Clone, PartialEq, Debug)]
    struct StoreType {
        val: i32,
    }

    let mut change_tracker = ChangeTracker::new(StoreType { val: 123 });
    let rx = change_tracker.get_changes(1);

    // Create a future to cause a change
    let cause_change = async move {
        change_tracker.modify(|scoped_store| {
            assert!((*scoped_store).val == 123);
            (*scoped_store).val += 1;
        });
    };

    futures::executor::block_on(cause_change);

    let check_change = rx.take(1).for_each(|(old_value, new_value)| {
        assert!(old_value.val == 123);
        assert!(new_value.val == 124);
        futures::future::ready(())
    });

    futures::executor::block_on(check_change);
}

#[test]
fn test_dropped_rx() {
    #[derive(Clone, PartialEq, Debug)]
    struct StoreType {
        val: i32,
    }

    let data_store = ChangeTracker::new(StoreType { val: 123 });
    let data_store_rc = Rc::new(RefCell::new(data_store));

    {
        let _rx = data_store_rc.borrow_mut().get_changes(1);
        // drop rx at end of this scope
    }

    let dsclone = data_store_rc.clone();
    // Create a future to cause a change
    let cause_change = async move {
        dsclone.borrow_mut().modify(|scoped_store| {
            assert!((*scoped_store).val == 123);
            (*scoped_store).val += 1;
        });
    };

    futures::executor::block_on(cause_change);

    assert!(data_store_rc.borrow().as_ref().val == 124);
}

#[test]
fn test_multiple_changes_no_rx() {
    #[derive(Clone, PartialEq, Debug)]
    struct StoreType {
        val: i32,
    }

    let data_store = ChangeTracker::new(StoreType { val: 123 });
    let data_store_rc = Rc::new(RefCell::new(data_store));

    let dsclone1 = data_store_rc.clone();
    let dsclone2 = data_store_rc.clone();

    // Create a future to cause a change
    let cause_change1 = async move {
        let mut data_store = dsclone1.borrow_mut();
        data_store.modify(|scoped_store| {
            (*scoped_store).val += 1;
        });
    };
    futures::executor::block_on(cause_change1);

    // Create a future to cause a change
    let cause_change2 = async move {
        let mut data_store = dsclone2.borrow_mut();
        data_store.modify(|scoped_store| {
            (*scoped_store).val += 1;
        });
    };
    futures::executor::block_on(cause_change2);
    assert!(data_store_rc.borrow().as_ref().val == 125);
}

#[test]
fn test_multithreaded_change_tracker() {
    use futures::task::SpawnExt;

    #[derive(Clone, PartialEq, Debug)]
    struct StoreType {
        val: i32,
    }

    let data_store = ChangeTracker::new(StoreType { val: 123 });
    let rx = data_store.get_changes(1);
    let data_store_arc = Arc::new(Mutex::new(data_store));
    let rx_printer = rx.take(1).for_each(|(old_value, new_value)| {
        assert!(old_value.val == 123);
        assert!(new_value.val == 124);
        futures::future::ready(())
    });

    // use multi-threaded runtime
    use futures::executor::ThreadPool; // use `cargo test --features "futures/thread-pool"`
    let rt = ThreadPool::new().unwrap();

    let dsclone = data_store_arc.clone();
    // Create a future to cause a change
    let cause_change = async move {
        let mut data_store = dsclone.lock();
        data_store.modify(|scoped_store| {
            assert!((*scoped_store).val == 123);
            (*scoped_store).val += 1;
        });
    };

    let do_all_fut = async move {
        cause_change.await;
        rx_printer.await;
    };

    let join_handle_fut = rt.spawn_with_handle(do_all_fut).unwrap();
    futures::executor::block_on(join_handle_fut);

    assert!(data_store_arc.lock().as_ref().val == 124);
}
