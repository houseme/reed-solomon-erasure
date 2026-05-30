extern crate alloc;

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use alloc::vec::Vec;
use core::alloc::Layout;
use smallvec::SmallVec;

const WORK_ALIGNMENT: usize = 64;

#[derive(Debug)]
pub(super) struct FlatWork {
    lanes: usize,
    lane_len: usize,
    ptr: *mut u8,
    len: usize,
    views: Vec<*mut [u8]>,
}

unsafe impl Send for FlatWork {}
unsafe impl Sync for FlatWork {}

impl FlatWork {
    pub(super) fn new(lanes: usize, lane_len: usize) -> Self {
        let len = lanes * lane_len;
        let layout = Layout::from_size_align(len, WORK_ALIGNMENT)
            .expect("FlatWork layout overflow");
        // SAFETY: layout is non-zero (lanes > 0, lane_len > 0).
        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        Self {
            lanes,
            lane_len,
            ptr,
            len,
            views: Vec::with_capacity(lanes),
        }
    }

    pub(super) fn lanes(&self) -> usize {
        self.lanes
    }

    pub(super) fn lane_len(&self) -> usize {
        self.lane_len
    }

    pub(super) fn lane(&self, idx: usize) -> &[u8] {
        debug_assert!(idx < self.lanes, "lane index {idx} out of bounds (lanes={})", self.lanes);
        let start = idx * self.lane_len;
        // SAFETY: start..start+lane_len is within allocated bounds, self.ptr is valid.
        unsafe { core::slice::from_raw_parts(self.ptr.add(start), self.lane_len) }
    }

    pub(super) fn lane_mut(&mut self, idx: usize) -> &mut [u8] {
        debug_assert!(idx < self.lanes, "lane index {idx} out of bounds (lanes={})", self.lanes);
        let start = idx * self.lane_len;
        // SAFETY: start..start+lane_len is within allocated bounds, self.ptr is valid,
        // and we have exclusive access through &mut self.
        unsafe { core::slice::from_raw_parts_mut(self.ptr.add(start), self.lane_len) }
    }

    /// Build lane view pointers into the cached `views` vector.
    /// Returns a mutable slice of `&mut [u8]` views covering `size` bytes per lane.
    pub(super) fn with_lane_views<R>(
        &mut self,
        lanes: usize,
        size: usize,
        f: impl FnOnce(&mut [&mut [u8]]) -> R,
    ) -> R {
        debug_assert!(size <= self.lane_len, "view size {size} exceeds lane_len {}", self.lane_len);
        debug_assert!(lanes <= self.lanes, "requested lanes {lanes} exceeds capacity {}", self.lanes);

        // Build view pointers. We use a SmallVec on the stack because the views
        // contain mutable references that cannot be cached across calls.
        let mut views: SmallVec<[&mut [u8]; 96]> = (0..lanes)
            .map(|i| {
                let start = i * self.lane_len;
                // SAFETY: each lane is at a distinct offset, no overlap.
                unsafe {
                    let ptr = self.ptr.add(start);
                    &mut *core::ptr::slice_from_raw_parts_mut(ptr, size)
                }
            })
            .collect();
        f(&mut views)
    }
}

impl Drop for FlatWork {
    fn drop(&mut self) {
        if self.len > 0 {
            let layout = Layout::from_size_align(self.len, WORK_ALIGNMENT)
                .expect("FlatWork layout overflow");
            // SAFETY: self.ptr was allocated with the same layout.
            unsafe { dealloc(self.ptr, layout) };
        }
    }
}
