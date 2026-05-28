use alloc::boxed::Box;
use alloc::vec::Vec;
use smallvec::SmallVec;

#[derive(Debug)]
pub(super) struct FlatWork {
    lanes: usize,
    lane_len: usize,
    buf: Box<[u8]>,
}

impl FlatWork {
    pub(super) fn new(lanes: usize, lane_len: usize) -> Self {
        Self {
            lanes,
            lane_len,
            buf: vec![0u8; lanes * lane_len].into_boxed_slice(),
        }
    }

    pub(super) fn lanes(&self) -> usize {
        self.lanes
    }

    pub(super) fn lane_len(&self) -> usize {
        self.lane_len
    }

    pub(super) fn lane(&self, idx: usize) -> &[u8] {
        let start = idx * self.lane_len;
        let end = start + self.lane_len;
        &self.buf[start..end]
    }

    pub(super) fn lane_mut(&mut self, idx: usize) -> &mut [u8] {
        let start = idx * self.lane_len;
        let end = start + self.lane_len;
        &mut self.buf[start..end]
    }

    pub(super) fn lane_views(&mut self, lanes: usize, size: usize) -> Vec<&mut [u8]> {
        self.buf
            .chunks_mut(self.lane_len)
            .take(lanes)
            .map(|lane| &mut lane[..size])
            .collect()
    }

    pub(super) fn with_lane_views<R>(
        &mut self,
        lanes: usize,
        size: usize,
        f: impl FnOnce(&mut [&mut [u8]]) -> R,
    ) -> R {
        let mut views: SmallVec<[&mut [u8]; 96]> = self
            .buf
            .chunks_mut(self.lane_len)
            .take(lanes)
            .map(|lane| &mut lane[..size])
            .collect();
        f(&mut views)
    }
}
