static mut PAGE_SIZE: usize = 0;
static mut PAGE_SIZE_BITS: usize = 0;

pub fn page_size() -> usize {
    let result = unsafe { PAGE_SIZE };

    if result != 0 {
        return result;
    }

    init_page_size();

    unsafe { PAGE_SIZE }
}

pub fn page_size_bits() -> usize {
    let result = unsafe { PAGE_SIZE_BITS };

    if result != 0 {
        return result;
    }

    init_page_size();

    unsafe { PAGE_SIZE_BITS }
}

fn init_page_size() {
    unsafe {
        PAGE_SIZE = determine_page_size();
        assert!((PAGE_SIZE & (PAGE_SIZE - 1)) == 0);

        PAGE_SIZE_BITS = log2(PAGE_SIZE);
    }
}

#[cfg(target_family = "unix")]
fn determine_page_size() -> usize {
    let val = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };

    if val <= 0 {
        panic!("could not determine page size.");
    }

    val as usize
}

#[cfg(target_family = "windows")]
fn determine_page_size() -> usize {
    use winapi::um::sysinfoapi::{GetSystemInfo, LPSYSTEM_INFO, SYSTEM_INFO};

    unsafe {
        let mut system_info: SYSTEM_INFO = std::mem::zeroed();
        GetSystemInfo(&mut system_info as LPSYSTEM_INFO);

        system_info.dwPageSize as usize
    }
}

/// determine log_2 of given value
fn log2(mut val: usize) -> usize {
    let mut log = 0;
    assert!(val <= u32::max_value() as usize);

    if (val & 0xFFFF0000) != 0 {
        val >>= 16;
        log += 16;
    }
    if val >= 256 {
        val >>= 8;
        log += 8;
    }
    if val >= 16 {
        val >>= 4;
        log += 4;
    }
    if val >= 4 {
        val >>= 2;
        log += 2;
    }

    log + (val >> 1)
}

#[test]
fn test_log2() {
    for i in 0..32 {
        assert_eq!(i, log2(1 << i));
    }
}
