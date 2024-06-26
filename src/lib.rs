//! Reactive change notifications using futures.
//!
//! The `ChangeTracker<T>` type wraps an owned value `T`. Changes to `T` are
//! done within a function or closure implementing `FnOnce(&mut T)`. When this
//! returns, any changes are sent to listeners using a `futures::Stream`.
//!
//! In slightly more detail, create a `ChangeTracker<T>` with
//! [`ChangeTracker::new(value: T)`](struct.ChangeTracker.html#method.new). This
//! will take ownership of the value of type `T`. You can then create a
//! `futures::Stream` (with
//! [`get_changes()`](struct.ChangeTracker.html#method.get_changes)) that emits
//! a tuple `(old_value, new_value)` of type `(T, T)` upon every change to the
//! owned value. The value can be changed with the
//! [`modify()`](struct.ChangeTracker.html#method.modify) method of
//! `ChangeTracker` and read using the `as_ref()` method from the `AsRef` trait.
//!
//! ## Example
//!
//! In this example, the functionality of `ChangeTracker` is shown.
//!
//! ```rust
//! use futures::stream::StreamExt;
//!
//! // Wrap an integer `with ChangeTracker`
//! let mut change_tracker = async_change_tracker::ChangeTracker::new( 123 );
//!
//! // Create an receiver that fires when the value changes. The channel size
//! // is 1, meaning at most one change can be buffered before backpressure
//! // is applied.
//! let rx = change_tracker.get_changes(1);
//!
//! // In this example take a single change and check that the old and new value
//! // are correct.
//! let rx_printer = rx.take(1).for_each(|(old_value, new_value)| {
//!     assert_eq!( old_value, 123);
//!     assert_eq!( new_value, 124);
//!     futures::future::ready(())
//! });
//!
//! // Now, check and then change the value.
//! change_tracker.modify(|mut_ref_value| {
//!     assert_eq!(*mut_ref_value, 123);
//!     *mut_ref_value += 1;
//! });
//!
//! // Wait until the stream is done. In this example, the stream ends due to
//! // the use of `.take(1)` prior to `for_each` above. In normal usage,
//! // typically the stream would finish for a different reason.
//! futures::executor::block_on(rx_printer);
//!
//! // Finally, check that the final value is as expected.
//! assert!(*change_tracker.as_ref() == 124);
//! ```
//!
//! ## Testing
//!
//! To test:
//!
//! ```text
//! cargo test
//! ```
#![deny(missing_docs)]

use futures::channel::mpsc;
use std::sync::{Arc, RwLock};

/// Tracks changes to data. Notifies listeners via a `futures::Stream`.
///
/// The data to be tracked is type `T`. The value of type `T` is wrapped in a
/// private field. The `AsRef` trait is implemented so `&T` can be obtained by
/// calling `as_ref()`. Read and write access can be gained by calling the
/// `modify` method.
///
/// Subscribe to changes by calling `get_changes`.
///
/// Note that this does not implement Clone because typically this is not what
/// you want. Rather, you should wrap ChangeTracker in `Arc<RwLock>` or similar.
///
/// See the module-level documentation for more information and a usage example.
pub struct ChangeTracker<T> {
    value: T,
    senders: Arc<RwLock<VecSender<T>>>,
}

type VecSender<T> = Vec<mpsc::Sender<(T, T)>>;

impl<T> ChangeTracker<T>
where
    T: Clone,
{
    /// Create a new `ChangeTracker` which takes ownership
    /// of the data of type `T`.
    pub fn new(value: T) -> Self {
        Self {
            value,
            senders: Arc::new(RwLock::new(Vec::new())),
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
        let mut senders = self.senders.write().unwrap();
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
        let new_value = self.value.clone();
        {
            let mut senders = self.senders.write().unwrap();
            let mut keep = vec![];
            for mut on_changed_tx in senders.drain(0..) {
                // TODO use .send() here?
                match on_changed_tx.start_send((orig.clone(), new_value.clone())) {
                    Ok(_) => {
                        keep.push(on_changed_tx);
                    }
                    Err(e) => {
                        if e.is_disconnected() {
                            tracing::trace!("receiver dropped");
                        } else {
                            tracing::trace!("error on start_send: {e}");
                            keep.push(on_changed_tx);
                        }
                    }
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
