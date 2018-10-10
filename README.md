# async-change-tracker - Reactive change notifications using futures. [![Version][version-img]][version-url] [![Doc][doc-img]][doc-url]

The main item of interest is the struct `ChangeTracker`. Create a new
`ChangeTracker` that takes ownership of an object of type `T`. You can then
create a futures::Stream (with `get_changes()`) that gets a new value upon
every change to the value. The value can be changed with the `modify()`
method of `ChangeTracker` and read using the `as_ref()` method.

This is the successor of
[raii-change-tracker](https://crates.io/crates/raii-change-tracker).

## Usage

```rust
extern crate async_change_tracker;
extern crate futures;
extern crate tokio;

use futures::Stream;

// Wrap an integer `with ChangeTracker`
let mut change_tracker = async_change_tracker::ChangeTracker::new( 123 );

// Create an receiver that fires when the value changes
let rx = change_tracker.get_changes(1);

// In this example, upon change, check the old and new value are correct.
let rx_printer = rx.for_each(|(old_value, new_value)| {
    assert_eq!( old_value, 123);
    assert_eq!( new_value, 124);
    // Here in this example, we return error to abort the stream.
    // In normal usage, typically an `Ok` value would be returned.
    futures::future::err(())
});

// Now, check and then change the value.
change_tracker.modify(|scoped_store| {
    assert_eq!(*scoped_store, 123);
    *scoped_store += 1;
});

// Wait until the stream is done.
match tokio::runtime::current_thread::block_on_all(rx_printer) {
    Ok(_) => panic!("should not get here"),
    Err(()) => (),
}

// Finally, check that the final value is as expected.
assert!(*change_tracker.as_ref() == 124);
```

[doc-img]: https://docs.rs/async-change-tracker/badge.svg
[doc-url]: https://docs.rs/async-change-tracker/
[version-img]: https://img.shields.io/crates/v/async-change-tracker.svg
[version-url]: https://crates.io/crates/async-change-tracker
