use std::{
    alloc::{GlobalAlloc, Layout, System},
    ptr::NonNull,
};

use crate::generated::fixed_size_constants::{BLOCK_ALIGN, BLOCK_SIZE};

use super::super::{Arena, ChunkFooter};

// Linux's system allocator supports high alignment allocations directly, so we just request what we want.
// We assume that other non-MacOS, non-Windows platforms also support high alignment allocations.

/// Layout of backing allocations.
const ALLOC_LAYOUT: Layout = match Layout::from_size_align(BLOCK_SIZE, BLOCK_ALIGN) {
    Ok(layout) => layout,
    Err(_) => unreachable!(),
};

const _: () = assert!(ALLOC_LAYOUT.size() > 0);

impl<const MIN_ALIGN: usize> Arena<MIN_ALIGN> {
    /// Construct a static-sized [`Arena`] backed by an allocation made via the [`System`] allocator.
    ///
    /// The returned [`Arena`] uses a single chunk of `BLOCK_SIZE` bytes, aligned on `BLOCK_ALIGN`.
    /// It cannot grow.
    ///
    /// Returns `None` if the allocation fails.
    ///
    /// See module-level docs for the rationale and platform-specific allocation strategy.
    pub fn new_fixed_size() -> Option<Self> {
        // Allocate block of memory aligned on `BLOCK_ALIGN`.
        // SAFETY: `ALLOC_LAYOUT` does not have zero size.
        let alloc_ptr = unsafe { System.alloc(ALLOC_LAYOUT) };
        let alloc_ptr = NonNull::new(alloc_ptr)?;

        debug_assert!(alloc_ptr.addr().get().is_multiple_of(BLOCK_ALIGN));

        // SAFETY:
        // * Region starting at `alloc_ptr` with `BLOCK_SIZE` bytes is the allocation we just made.
        // * `alloc_ptr` has high alignment (`BLOCK_ALIGN`).
        // * `BLOCK_SIZE` is large and a multiple of 16.
        // * `alloc_ptr` has permission for writes.
        let arena = unsafe { Self::from_raw_parts(alloc_ptr, BLOCK_SIZE, alloc_ptr, ALLOC_LAYOUT) };

        Some(arena)
    }
}

/// Deallocate the chunk whose footer is pointed to by `footer_ptr`, when the chunk is fixed size
/// (created via `Arena::from_raw_parts` or `Arena::new_fixed_size`).
///
/// `dealloc_chunk` in `drop` module delegates to this function when chunk's `is_fixed_size` flag is set.
/// `free_fixed_size_allocator` in `pool/fixed_size.rs` also uses this function for deallocation.
///
/// # SAFETY
///
/// * `footer_ptr` must point to a valid `ChunkFooter`.
/// * `ChunkFooter` must be for a fixed size chunk (created via `Arena::from_raw_parts` or `Arena::new_fixed_size`).
pub unsafe fn dealloc_fixed_size_arena_chunk(footer_ptr: NonNull<ChunkFooter>) {
    // Create `&ChunkFooter` reference within a block, to ensure the reference is not live
    // when we deallocate the chunk's memory (which includes the `ChunkFooter`)
    let (backing_alloc_ptr, layout, is_fixed_size) = {
        // SAFETY: Caller guarantees that `footer_ptr` points to a valid `ChunkFooter`
        let footer = unsafe { footer_ptr.as_ref() };
        (footer.backing_alloc_ptr.as_ptr(), footer.layout, footer.is_fixed_size)
    };

    debug_assert!(
        is_fixed_size,
        "Only fixed-size allocators should be passed to `dealloc_fixed_size_arena_chunk` to deallocate"
    );

    // SAFETY: Each `ChunkFooter`'s `backing_alloc_ptr` and `layout` describe its backing allocation.
    // Caller guarantees `is_fixed_size` is `true`, so backing allocation was made via `System` allocator.
    unsafe { System.dealloc(backing_alloc_ptr, layout) };
}
