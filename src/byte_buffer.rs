use std::convert::TryFrom;
use std::mem::ManuallyDrop;

use crate::FrameworkByteBuffer;

impl Default for FrameworkByteBuffer {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
            length: 0,
            capacity: 0,
        }
    }
}

impl FrameworkByteBuffer {
    pub(crate) fn from_vec(bytes: Vec<u8>) -> Self {
        let length = i32::try_from(bytes.len()).expect("buffer length overflowed i32");
        let capacity = i32::try_from(bytes.capacity()).expect("buffer capacity overflowed i32");
        let mut bytes = ManuallyDrop::new(bytes);

        Self {
            ptr: bytes.as_mut_ptr(),
            length,
            capacity,
        }
    }

    pub(crate) unsafe fn destroy(self) {
        if self.ptr.is_null() {
            return;
        }

        let length = usize::try_from(self.length).expect("negative buffer length");
        let capacity = usize::try_from(self.capacity).expect("negative buffer capacity");
        drop(Vec::from_raw_parts(self.ptr, length, capacity));
    }
}
