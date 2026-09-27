use std::arch::asm;

pub(super) fn find_percent(bytes: &[u8]) -> Option<usize> {
    // A zero-count scan would leave the zero flag undefined.
    if bytes.is_empty() {
        return None;
    }
    let cursor = bytes.as_ptr();
    let mut remaining = bytes.len();
    let matched: u8;

    // SAFETY: `bytes` points to `bytes.len()` initialized, readable bytes.
    // RCX starts at that length, so `repne scasb` reads at most those
    // bytes. `cld` makes RDI advance forward by one per read. The search
    // does not write through RDI or retain the pointer. A match consumes
    // one byte, making `len - remaining - 1` its in-bounds index. The
    // empty-slice check above prevents a zero-count scan.
    unsafe {
        asm!(
            include_str!("find_percent.asm"),
            inout("rdi") cursor => _,
            inout("rcx") remaining,
            inout("al") b'%' => matched,
            options(nostack, readonly),
        );
    }

    if matched != 0 {
        Some(bytes.len() - remaining - 1)
    } else {
        None
    }
}
