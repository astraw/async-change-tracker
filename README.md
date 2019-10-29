# async-change-tracker

Reactive change notifications using futures.

The `ChangeTracker<T>` type allows changing an owned value `T` using
closures and notifies listeners just after the closure completes.

Creating a new with [`ChangeTracker::new(value:
T)`](struct.ChangeTracker.html#method.new) will take ownership of the value
of type `T`. You can then create a futures::Stream (with
[`get_changes()`](struct.ChangeTracker.html#method.get_changes)) that emits
a tuple `(old_value, new_value)` of type `(T, T)` upon every change to the
owned value. The value can be changed with the
[`modify()`](struct.ChangeTracker.html#method.modify) method of
`ChangeTracker` and read using the `as_ref()` method from the `AsRef` trait.

### Example

In this example, the functionality of `ChangeTracker` is shown.

```rust
use futures::stream::StreamExt;

// Wrap an integer `with ChangeTracker`
let mut change_tracker = async_change_tracker::ChangeTracker::new( 123 );

// Create an receiver that fires when the value changes. The channel size
// is 1, meaning at most one change can be buffered before backpressure
// is applied.
let rx = change_tracker.get_changes(1);

// In this example take a single change and check that the old and new value
// are correct.
let rx_printer = rx.take(1).for_each(|(old_value, new_value)| {
    assert_eq!( old_value, 123);
    assert_eq!( new_value, 124);
    futures::future::ready(())
});

// Now, check and then change the value.
change_tracker.modify(|scoped_store| {
    assert_eq!(*scoped_store, 123);
    *scoped_store += 1;
});

// Wait until the stream is done. In this example, the stream ends due to
// the error we return in the `for_each` closure above. In normal usage,
// typically the stream would finish for a different reason.
futures::executor::block_on(rx_printer);

// Finally, check that the final value is as expected.
assert!(*change_tracker.as_ref() == 124);
```

License: MIT/Apache-2.0
