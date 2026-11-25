//! Fixed-size storage blocks whose slots never move.

// Storage that grows without moving its values needs raw memory. Each operation on it states what
// its caller must uphold.
#![allow(unsafe_code)]

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

/// The number of slots in one storage block.
// About 64 KiB per block for a 128-byte value.
pub const BLOCK_LEN: usize = 512;

/// A block of [`BLOCK_LEN`] slots, each holding a value or uninitialised.
///
/// A block is never moved or resized, so a slot's address is fixed for the block's life. Writing a
/// slot takes `&self`, so references to other slots of the block stay valid.
///
/// Every slot method panics for a slot index of [`BLOCK_LEN`] or more.
#[repr(transparent)]
pub(crate) struct Block<T>([UnsafeCell<MaybeUninit<T>>; BLOCK_LEN]);

impl<T> Block<T> {
    /// Allocates a block whose slots are all uninitialised.
    pub(crate) fn new() -> Box<Self> {
        let layout = core::alloc::Layout::new::<Self>();
        if layout.size() == 0 {
            // The allocator may not be asked for zero bytes.
            return Box::new(Self(core::array::from_fn(|_| {
                UnsafeCell::new(MaybeUninit::uninit())
            })));
        }
        // SAFETY: the layout has a non-zero size, checked directly above. The allocation is
        // handed straight to `Box::from_raw`, which takes ownership of exactly the layout
        // `Layout::new::<Self>()` describes, so the eventual deallocation matches this
        // allocation. Every slot of `Self` is a `MaybeUninit`, for which any bit pattern,
        // including the uninitialised one the allocator returns, is a valid value.
        let pointer = unsafe { std::alloc::alloc(layout) }.cast::<Self>();
        if pointer.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        // SAFETY: `pointer` is non-null, checked directly above. It was produced by the global
        // allocator with the layout of `Self` and points at a valid `Self` by the argument above,
        // and it is not aliased, having just been allocated.
        unsafe { Box::from_raw(pointer) }
    }

    /// Places a value in a slot.
    ///
    /// # Safety
    ///
    /// The slot must be uninitialised, and no reference into it may be live.
    pub(crate) unsafe fn write(&self, slot: usize, value: T) {
        // SAFETY: the pointer comes from a cell of this block, so it is valid and aligned for a
        // write of `MaybeUninit<T>`, and by the caller's promise nothing else refers to this slot
        // for the duration of the write.
        unsafe { self.0[slot].get().write(MaybeUninit::new(value)) };
    }

    /// Returns the value in a slot.
    ///
    /// # Safety
    ///
    /// The slot must hold a value, and must not be emptied while the returned reference lives.
    pub(crate) unsafe fn get(&self, slot: usize) -> &T {
        // SAFETY: the slot is initialised by the caller's promise. The reference is derived from
        // `&self`, so it borrows the block and not any unique access to it, and the caller's
        // promise rules out a concurrent write to this slot.
        unsafe { (*self.0[slot].get()).assume_init_ref() }
    }

    /// Returns the value in a slot for modification.
    ///
    /// # Safety
    ///
    /// The slot must hold a value.
    pub(crate) unsafe fn get_mut(&mut self, slot: usize) -> &mut T {
        // SAFETY: the slot is initialised by the caller's promise, and the reference is derived
        // from `&mut self`, which is itself proof that no other reference into the block is live.
        unsafe { (*self.0[slot].get()).assume_init_mut() }
    }

    /// Moves the value out of a slot, leaving the slot uninitialised.
    ///
    /// # Safety
    ///
    /// The slot must hold a value, and no reference into it may be live.
    pub(crate) unsafe fn take(&self, slot: usize) -> T {
        // SAFETY: the slot is initialised by the caller's promise, so the pointer is valid and
        // aligned for a read of `T`. The caller promises no reference into the slot is live, so
        // moving the value out leaves nothing dangling, and treats the slot as uninitialised
        // afterwards, so the value is not read twice.
        unsafe { self.0[slot].get().read().assume_init() }
    }

    /// Drops the value in a slot in place, leaving the slot uninitialised.
    ///
    /// # Safety
    ///
    /// The slot must hold a value, and no reference into it may be live.
    pub(crate) unsafe fn drop_value(&self, slot: usize) {
        // SAFETY: as for `take`, except that the value is destroyed where it lies. The caller's
        // promises are the same and rule out a double drop.
        unsafe { (*self.0[slot].get()).assume_init_drop() };
    }
}

// SAFETY: every method that reaches a slot is `unsafe` and demands the caller rule out concurrent
// access to that slot, so safe code holding `&Block<T>` can observe nothing at all. The only value
// that can be obtained from a shared block is a `&T`, so sharing a block across threads requires
// exactly what sharing a `&T` requires.
unsafe impl<T: Sync> Sync for Block<T> {}

// SAFETY: a block owns its slots and nothing else, so moving one to another thread moves the
// values it holds and nothing more.
unsafe impl<T: Send> Send for Block<T> {}
