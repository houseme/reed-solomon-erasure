extern crate alloc;

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::fmt;
use core::iter::FromIterator;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::slice;

pub const SHARD_ALIGNMENT: usize = 64;

pub struct AlignedShard {
    ptr: NonNull<u8>,
    len: usize,
}

impl AlignedShard {
    pub fn new_zeroed(len: usize) -> Self {
        if len == 0 {
            return Self {
                ptr: NonNull::dangling(),
                len: 0,
            };
        }

        let layout = Layout::from_size_align(len, SHARD_ALIGNMENT)
            .expect("aligned shard layout must be valid");
        // SAFETY: `layout` is constructed above and `alloc_zeroed` returns a
        // uniquely owned allocation or null on OOM.
        let ptr = unsafe { alloc_zeroed(layout) };
        let ptr = NonNull::new(ptr).unwrap_or_else(|| handle_alloc_error(layout));

        Self { ptr, len }
    }

    pub fn from_slice(data: &[u8]) -> Self {
        let mut shard = Self::new_zeroed(data.len());
        shard.as_mut().copy_from_slice(data);
        shard
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_ptr(&self) -> *const u8 {
        self.ptr.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr.as_ptr()
    }
}

impl Clone for AlignedShard {
    fn clone(&self) -> Self {
        Self::from_slice(self.as_ref())
    }
}

impl fmt::Debug for AlignedShard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AlignedShard")
            .field("len", &self.len)
            .field("alignment", &SHARD_ALIGNMENT)
            .finish()
    }
}

impl Drop for AlignedShard {
    fn drop(&mut self) {
        if self.len == 0 {
            return;
        }

        let layout = Layout::from_size_align(self.len, SHARD_ALIGNMENT)
            .expect("aligned shard layout must be valid");
        // SAFETY: `self.ptr` was allocated from `alloc_zeroed` with the same
        // layout in `new_zeroed`, and this value owns the allocation uniquely.
        unsafe {
            dealloc(self.ptr.as_ptr(), layout);
        }
    }
}

impl Deref for AlignedShard {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl DerefMut for AlignedShard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl AsRef<[u8]> for AlignedShard {
    fn as_ref(&self) -> &[u8] {
        // SAFETY: `self.ptr` points to `self.len` bytes owned by this value,
        // or is a non-null dangling pointer when `self.len == 0`.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
}

impl AsMut<[u8]> for AlignedShard {
    fn as_mut(&mut self) -> &mut [u8] {
        // SAFETY: same allocation guarantees as `as_ref`, with unique mutable
        // access enforced by `&mut self`.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

impl FromIterator<u8> for AlignedShard {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        let bytes: Vec<u8> = iter.into_iter().collect();
        Self::from_slice(&bytes)
    }
}

// SAFETY: `AlignedShard` owns its allocation and moving it across threads does
// not violate aliasing. Shared access only exposes immutable `u8` slices.
unsafe impl Send for AlignedShard {}
// SAFETY: shared references expose immutable bytes, and mutable access still
// requires `&mut self`.
unsafe impl Sync for AlignedShard {}

pub fn alloc_aligned_shards(total_shards: usize, shard_len: usize) -> Vec<AlignedShard> {
    (0..total_shards)
        .map(|_| AlignedShard::new_zeroed(shard_len))
        .collect()
}

impl crate::ReedSolomon<super::Field> {
    pub fn alloc_aligned(&self, shard_len: usize) -> Vec<AlignedShard> {
        alloc_aligned_shards(self.total_shard_count(), shard_len)
    }
}
