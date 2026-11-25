//! A global allocator that counts the allocations of the threads that ask it to.

// `GlobalAlloc` is an unsafe trait, so implementing it takes unsafe code.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// The system allocator, counting the allocations and reallocations made inside [`allocations`]
/// on the calling thread.
///
/// A test binary installs it with `#[global_allocator]`. Allocations of other threads are not
/// counted.
#[derive(Clone, Copy, Debug, Default)]
pub struct CountingAlloc;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static COUNTED: Cell<u64> = const { Cell::new(0) };
}

// SAFETY: every call is forwarded to the system allocator unchanged, and counting touches only
// thread-local cells, which allocate nothing.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count();
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, size) }
    }
}

fn count() {
    // A thread that is being torn down has no counter left.
    let _ = COUNTING.try_with(|on| {
        if on.get() {
            COUNTED.with(|counted| counted.set(counted.get() + 1));
        }
    });
}

/// Returns how many allocations and reallocations `then` makes on this thread.
///
/// The count is 0 unless the binary's global allocator is [`CountingAlloc`].
pub fn allocations(then: impl FnOnce()) -> u64 {
    COUNTED.with(|counted| counted.set(0));
    COUNTING.with(|on| on.set(true));
    then();
    COUNTING.with(|on| on.set(false));
    COUNTED.with(Cell::get)
}
