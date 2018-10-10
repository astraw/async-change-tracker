//! Reactive change notifications using futures.
//!
//! This is the successor of
//! [raii-change-tracker](https://crates.io/crates/raii-change-tracker).
//!
//! The main item of interest is [`ChangeTracker`](struct.ChangeTracker.html).
//! Creating a new with [`ChangeTracker::new(value:
//! T)`](struct.ChangeTracker.html#method.new) will take ownership of the value
//! of type `T`. You can then create a futures::Stream (with
//! [`get_changes()`](struct.ChangeTracker.html#method.get_changes)) that emits
//! a tuple `(old_value, new_value)` of type `(T, T)` upon every change to the
//! owned value. The value can be changed with the
//! [`modify()`](struct.ChangeTracker.html#method.modify) method of
//! `ChangeTracker` and read using the `as_ref()` method from the `AsRef` trait.
//!
//! ## Example
//!
//! In this example, all the functionality of `ChangeTracker` is shown. A
//! [`tokio`](https://crates.io/crates/tokio) runtime is used to execute the
//! futures.
//!
//! ```rust
//! extern crate async_change_tracker;
//! extern crate futures;
//! extern crate tokio;
//!
//! use futures::Stream;
//!
//! // Wrap an integer `with ChangeTracker`
//! let mut change_tracker = async_change_tracker::ChangeTracker::new( 123 );
//!
//! // Create an receiver that fires when the value changes
//! let rx = change_tracker.get_changes(1);
//!
//! // In this example, upon change, check the old and new value are correct.
//! let rx_printer = rx.for_each(|(old_value, new_value)| {
//!     assert_eq!( old_value, 123);
//!     assert_eq!( new_value, 124);
//!     // Here in this example, we return error to abort the stream.
//!     // In normal usage, typically an `Ok` value would be returned.
//!     futures::future::err(())
//! });
//!
//! // Now, check and then change the value.
//! change_tracker.modify(|scoped_store| {
//!     assert_eq!(*scoped_store, 123);
//!     *scoped_store += 1;
//! });
//!
//! // Wait until the stream is done. In this example, the stream ends due to
//! // the error we return in the `for_each` closure above. In normal usage,
//! // typically the stream would finish for a different reason.
//! match tokio::runtime::current_thread::block_on_all(rx_printer) {
//!     Ok(_) => panic!("should not get here"),
//!     Err(()) => (),
//! }
//!
//! // Finally, check that the final value is as expected.
//! assert!(*change_tracker.as_ref() == 124);
//! ```
#![deny(missing_docs)]

#[macro_use]
extern crate log;
extern crate futures;
extern crate parking_lot;

use futures::sync::mpsc;
use futures::Sink;
use parking_lot::Mutex;
use std::sync::Arc;

/// Tracks changes to data. Notifies listeners via a `futures::Stream`.
///
/// The data to be tracked is type `T`. The value of type `T` is wrapped in a
/// private field. The `AsRef` trait is implemented so `&T` can be obtained by
/// calling `as_ref()`. Read and write access can be gained by calling the
/// `modify` method.
///
/// Subsribe to changes by calling `get_changes`.
///
/// See the module-level documentation for more information and a usage example.
pub struct ChangeTracker<T> {
    value: T,
    senders: Arc<Mutex<Vec<mpsc::Sender<(T, T)>>>>,
}

impl<T> ChangeTracker<T>
where
    T: Clone,
{
    /// Create a new `ChangeTracker` which takes ownership
    /// of the data of type `T`.
    pub fn new(value: T) -> Self {
        Self {
            value,
            senders: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Returns a `futures::Stream` that emits a message when a change occurs
    ///
    /// The capacity of the underlying channel is specified with the `capacity`
    /// argument.
    ///
    /// To remove a listener, drop the Receiver.
    pub fn get_changes(&self, capacity: usize) -> mpsc::Receiver<(T, T)> {
        let (tx, rx) = mpsc::channel(capacity);
        let mut senders = self.senders.lock();
        senders.push(tx);
        rx
    }

    /// Modify the data value, notifying listeners upon change.
    pub fn modify<F>(&mut self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let orig = self.value.clone();
        f(&mut self.value);
        let newval = self.value.clone();
        {
            let mut senders = self.senders.lock();
            let mut keep = vec![];
            for mut on_changed_tx in senders.drain(0..) {
                // TODO use .send() here?
                match on_changed_tx.start_send((orig.clone(), newval.clone())) {
                    Ok(_) => keep.push(on_changed_tx),
                    Err(_) => trace!("receiver dropped"),
                }
            }
            senders.extend(keep);
        }
    }
}

impl<T> AsRef<T> for ChangeTracker<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}
