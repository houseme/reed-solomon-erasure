extern crate alloc;

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use core::alloc::Layout;
use smallvec::SmallVec;

const WORK_ALIGNMENT: usize = 64;

/// Flat allocation of u16 work lanes for GF16 Leopard encode/decode.
///
/// Same pattern as the GF8 `FlatWork` but with `u16` elements.
#[derive(Debug)]
pub(crate) struct FlatWork16 {
    lanes: usize,
    lane_len: usize,
    ptr: *mut u16,
    len_elems: usize,
}

unsafe impl Send for FlatWork16 {}
unsafe impl Sync for FlatWork16 {}

impl FlatWork16 {
    pub(crate) fn new(lanes: usize, lane_len: usize) -> Self {
        let len_elems = lanes * lane_len;
        let len_bytes = len_elems * core::mem::size_of::<u16>();
        let layout =
            Layout::from_size_align(len_bytes, WORK_ALIGNMENT).expect("FlatWork16 layout overflow");
        let ptr = unsafe { alloc_zeroed(layout) }.cast::<u16>();
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        Self {
            lanes,
            lane_len,
            ptr,
            len_elems,
        }
    }

    pub(crate) fn can_reuse(&self, lanes: usize, lane_len: usize) -> bool {
        self.lanes == lanes && self.lane_len == lane_len
    }

    pub(crate) fn lanes(&self) -> usize {
        self.lanes
    }

    pub(crate) fn lane(&self, idx: usize) -> &[u16] {
        debug_assert!(idx < self.lanes);
        let start = idx * self.lane_len;
        unsafe { core::slice::from_raw_parts(self.ptr.add(start), self.lane_len) }
    }

    pub(crate) fn lane_mut(&mut self, idx: usize) -> &mut [u16] {
        debug_assert!(idx < self.lanes);
        let start = idx * self.lane_len;
        unsafe { core::slice::from_raw_parts_mut(self.ptr.add(start), self.lane_len) }
    }

    /// Build lane view pointers into the flat buffer.
    pub(crate) fn with_lane_views<R>(
        &mut self,
        lanes: usize,
        size: usize,
        f: impl FnOnce(&mut [&mut [u16]]) -> R,
    ) -> R {
        debug_assert!(size <= self.lane_len);
        debug_assert!(lanes <= self.lanes);

        let mut views: SmallVec<[&mut [u16]; 96]> = (0..lanes)
            .map(|i| {
                let start = i * self.lane_len;
                unsafe {
                    let ptr = self.ptr.add(start);
                    &mut *core::ptr::slice_from_raw_parts_mut(ptr, size)
                }
            })
            .collect();
        f(&mut views)
    }
}

impl Drop for FlatWork16 {
    fn drop(&mut self) {
        if self.len_elems > 0 {
            let len_bytes = self.len_elems * core::mem::size_of::<u16>();
            let layout = Layout::from_size_align(len_bytes, WORK_ALIGNMENT)
                .expect("FlatWork16 layout overflow");
            unsafe { dealloc(self.ptr.cast::<u8>(), layout) };
        }
    }
}
