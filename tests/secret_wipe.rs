//! A dropped secret's buffers are zero by the time they are freed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use blobfig::Value;

const SIZE: usize = 4099;

/// The secret's buffer, inspected once, when it is freed.
static TARGET: AtomicPtr<u8> = AtomicPtr::new(ptr::null_mut());

/// Whether the target was all zero when it was freed.
static FREED_ZEROED: AtomicBool = AtomicBool::new(false);

/// Checks the target before freeing it, while it is still allocated. Every
/// test in this binary shares it and the statics above, so this file holds
/// only the one test.
struct Inspecting;

unsafe impl GlobalAlloc for Inspecting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
        // Clearing the target on its first match keeps a block that later
        // reuses the address from being read.
        let is_target = TARGET
            .compare_exchange(block, ptr::null_mut(), Ordering::SeqCst, Ordering::SeqCst)
            .is_ok();
        if is_target && layout.size() == SIZE {
            // SAFETY: the block is still allocated, and the test wrote all
            // `SIZE` bytes of it.
            let bytes = unsafe { slice::from_raw_parts(block, SIZE) };
            FREED_ZEROED.store(bytes.iter().all(|byte| *byte == 0), Ordering::SeqCst);
        }
        // SAFETY: forwarded with the caller's guarantees.
        unsafe { System.dealloc(block, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Inspecting = Inspecting;

#[test]
fn a_dropped_secret_is_zeroed_before_it_is_freed() {
    let mut secret = String::from_utf8(vec![b's'; SIZE]).expect("ASCII is UTF-8");
    TARGET.store(secret.as_mut_ptr(), Ordering::SeqCst);
    drop(Value::secret(secret));
    assert!(FREED_ZEROED.load(Ordering::SeqCst));
}
