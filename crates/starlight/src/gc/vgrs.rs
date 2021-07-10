use libc::c_uint;

// Client requests use a magic instruction sequence which differs
// by operating system and CPU architecture.  The `arch` modules
// define a single function `request`, and the rest of the code
// here is platform independent.
//
// The magic instructions as well as the values in `enums` are
// considered a stable ABI, according to `valgrind.h`.

#[cfg(all(
    target_arch = "x86_64",
    any(target_os = "linux", target_os = "macos", target_os = "freebsd")
))]
#[path = "arch/x86_64-linux-macos.rs"]
mod arch {

    #[allow(unused_mut)]
    #[inline(always)]
    pub unsafe fn request(
        default: usize,
        request: usize,
        arg1: usize,
        arg2: usize,
        arg3: usize,
        arg4: usize,
        arg5: usize,
    ) -> usize {
        let args: [usize; 6] = [request, arg1, arg2, arg3, arg4, arg5];
        let mut result: usize;

        // Valgrind notices this magic instruction sequence and interprets
        // it as a kind of hypercall.  When not running under Valgrind,
        // the instructions do nothing and `default` is returned.
        llvm_asm!("
        rolq $$3,  %rdi
        rolq $$13, %rdi
        rolq $$61, %rdi
        rolq $$51, %rdi
        xchgq %rbx, %rbx"

        : "={rdx}"(result)
        : "{rax}"(args.as_ptr()), "0"(default)
        : "cc", "memory"
        : "volatile");

        result
    }
}

#[cfg(all(
    target_arch = "x86",
    any(target_os = "linux", target_os = "macos", target_os = "freebsd")
))]
#[path = "arch/x86-linux-macos.rs"]
mod arch {
    #[allow(unused_mut)]
    #[inline(always)]
    pub unsafe fn request(
        default: uint,
        request: uint,
        arg1: uint,
        arg2: uint,
        arg3: uint,
        arg4: uint,
        arg5: uint,
    ) -> usize {
        let args: [uint; 6] = [request, arg1, arg2, arg3, arg4, arg5];
        let mut result: uint;

        // Valgrind notices this magic instruction sequence and interprets
        // it as a kind of hypercall.  When not running under Valgrind,
        // the instructions do nothing and `default` is returned.
        llvm_asm!("
        roll $$3,  %edi
        roll $$13, %edi
        roll $$29, %edi
        roll $$19, %edi
        xchgl %ebx, %ebx"

        : "={edx}"(result)
        : "{eax}"(args.as_ptr()), "0"(default)
        : "cc", "memory"
        : "volatile");

        result
    }
}

mod enums {
    #![allow(non_camel_case_types, dead_code)]

    pub use self::Vg_CallgrindClientRequest::*;
    pub use self::Vg_ClientRequest::*;
    pub use self::Vg_DRDClientRequest::*;
    pub use self::Vg_MemCheckClientRequest::*;
    pub use self::Vg_TCheckClientRequest::*;

    macro_rules! VG_USERREQ_TOOL_BASE ( ($a:expr, $b:expr) => (
    ((($a as isize) & 0xff) << 24)
  | ((($b as isize) & 0xff) << 16)));

    #[repr(C)]
    pub enum Vg_ClientRequest {
        VG_USERREQ__RUNNING_ON_VALGRIND = 0x1001,
        VG_USERREQ__DISCARD_TRANSLATIONS = 0x1002,
        VG_USERREQ__CLIENT_CALL0 = 0x1101,
        VG_USERREQ__CLIENT_CALL1 = 0x1102,
        VG_USERREQ__CLIENT_CALL2 = 0x1103,
        VG_USERREQ__CLIENT_CALL3 = 0x1104,
        VG_USERREQ__COUNT_ERRORS = 0x1201,
        VG_USERREQ__GDB_MONITOR_COMMAND = 0x1202,
        VG_USERREQ__MALLOCLIKE_BLOCK = 0x1301,
        VG_USERREQ__RESIZEINPLACE_BLOCK = 0x130b,
        VG_USERREQ__FREELIKE_BLOCK = 0x1302,
        VG_USERREQ__CREATE_MEMPOOL = 0x1303,
        VG_USERREQ__DESTROY_MEMPOOL = 0x1304,
        VG_USERREQ__MEMPOOL_ALLOC = 0x1305,
        VG_USERREQ__MEMPOOL_FREE = 0x1306,
        VG_USERREQ__MEMPOOL_TRIM = 0x1307,
        VG_USERREQ__MOVE_MEMPOOL = 0x1308,
        VG_USERREQ__MEMPOOL_CHANGE = 0x1309,
        VG_USERREQ__MEMPOOL_EXISTS = 0x130a,
        VG_USERREQ__PRINTF = 0x1401,
        VG_USERREQ__PRINTF_BACKTRACE = 0x1402,
        VG_USERREQ__PRINTF_VALIST_BY_REF = 0x1403,
        VG_USERREQ__PRINTF_BACKTRACE_VALIST_BY_REF = 0x1404,
        VG_USERREQ__STACK_REGISTER = 0x1501,
        VG_USERREQ__STACK_DEREGISTER = 0x1502,
        VG_USERREQ__STACK_CHANGE = 0x1503,
        VG_USERREQ__LOAD_PDB_DEBUGINFO = 0x1601,
        VG_USERREQ__MAP_IP_TO_SRCLOC = 0x1701,
        VG_USERREQ__CHANGE_ERR_DISABLEMENT = 0x1801,
    }

    #[repr(C)]
    pub enum Vg_MemCheckClientRequest {
        VG_USERREQ__MAKE_MEM_NOACCESS = VG_USERREQ_TOOL_BASE!('M', 'C'),
        VG_USERREQ__MAKE_MEM_UNDEFINED,
        VG_USERREQ__MAKE_MEM_DEFINED,
        VG_USERREQ__DISCARD,
        VG_USERREQ__CHECK_MEM_IS_ADDRESSABLE,
        VG_USERREQ__CHECK_MEM_IS_DEFINED,
        VG_USERREQ__DO_LEAK_CHECK,
        VG_USERREQ__COUNT_LEAKS,
        VG_USERREQ__GET_VBITS,
        VG_USERREQ__SET_VBITS,
        VG_USERREQ__CREATE_BLOCK,
        VG_USERREQ__MAKE_MEM_DEFINED_IF_ADDRESSABLE,
        VG_USERREQ__COUNT_LEAK_BLOCKS,
    }

    #[repr(C)]
    pub enum Vg_CallgrindClientRequest {
        VG_USERREQ__DUMP_STATS = VG_USERREQ_TOOL_BASE!('C', 'T'),
        VG_USERREQ__ZERO_STATS,
        VG_USERREQ__TOGGLE_COLLECT,
        VG_USERREQ__DUMP_STATS_AT,
        VG_USERREQ__START_INSTRUMENTATION,
        VG_USERREQ__STOP_INSTRUMENTATION,
    }

    #[repr(C)]
    pub enum Vg_TCheckClientRequest {
        VG_USERREQ__HG_CLEAN_MEMORY = VG_USERREQ_TOOL_BASE!('H', 'G'),
        _Vg_TCheckClientRequest_dummy, // suppress error about univariant enum repr
    }

    #[repr(C)]
    pub enum Vg_DRDClientRequest {
        // Binary compatible with the similar Helgrind request above
        VG_USERREQ__DRD_CLEAN_MEMORY = VG_USERREQ_TOOL_BASE!('H', 'G'),

        VG_USERREQ__DRD_GET_VALGRIND_THREAD_ID = VG_USERREQ_TOOL_BASE!('D', 'R'),
        VG_USERREQ__DRD_GET_DRD_THREAD_ID,
        VG_USERREQ__DRD_START_SUPPRESSION,
        VG_USERREQ__DRD_FINISH_SUPPRESSION,
        VG_USERREQ__DRD_START_TRACE_ADDR,
        VG_USERREQ__DRD_STOP_TRACE_ADDR,
        VG_USERREQ__DRD_RECORD_LOADS,
        VG_USERREQ__DRD_RECORD_STORES,
        VG_USERREQ__DRD_SET_THREAD_NAME,
    }
}

// We can interpret the result of a client request as any of
// these Rust types.
#[doc(hidden)]
trait FromUsize {
    fn from_usize(x: usize) -> Self;
}

impl FromUsize for usize {
    fn from_usize(x: usize) -> usize {
        x
    }
}

impl FromUsize for () {
    fn from_usize(_: usize) -> () {}
}

impl FromUsize for *const () {
    fn from_usize(x: usize) -> *const () {
        x as *const ()
    }
}

impl FromUsize for c_uint {
    fn from_usize(x: usize) -> c_uint {
        x as c_uint
    }
}

impl<T: FromUsize> FromUsize for Option<T> {
    fn from_usize(x: usize) -> Option<T> {
        match x {
            0 => None,
            _ => Some(FromUsize::from_usize(x)),
        }
    }
}

// Build a wrapper function of a given type.  We enumerate every arity
// because recursive macros with delimited lists don't work very well.
macro_rules! wrap (
    ($nr:ident => fn $name:ident ( ) -> $t_ret:ty) => (
        #[inline(always)]
        pub unsafe fn $name() -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize, 0, 0, 0, 0, 0))
        }
    );

    ($nr:ident => fn $name:ident ( $a1:ident : $t1:ty ) -> $t_ret:ty) => (
        #[inline(always)]
        pub unsafe fn $name($a1: $t1) -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize, $a1 as usize, 0, 0, 0, 0))
        }
    );

    ($nr:ident => fn $name:ident ( $a1:ident : $t1:ty , $a2:ident : $t2:ty ) -> $t_ret:ty) => (
        #[inline(always)]
        pub unsafe fn $name($a1: $t1, $a2: $t2) -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize, $a1 as usize, $a2 as usize, 0, 0, 0))
        }
    );

    ($nr:ident => fn $name:ident ( $a1:ident : $t1:ty , $a2:ident : $t2:ty,
            $a3:ident : $t3:ty ) -> $t_ret:ty ) => (
        #[inline(always)]
        pub unsafe fn $name($a1: $t1, $a2: $t2, $a3: $t3) -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize,
                $a1 as usize, $a2 as usize, $a3 as usize, 0, 0))
        }
    );

    ($nr:ident => fn $name:ident ( $a1:ident : $t1:ty , $a2:ident : $t2:ty,
            $a3:ident : $t3:ty, $a4:ident : $t4:ty ) -> $t_ret:ty) => (
        #[inline(always)]
        pub unsafe fn $name($a1: $t1, $a2: $t2, $a3: $t3, $a4: $t4) -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize,
                $a1 as usize, $a2 as usize, $a3 as usize, $a4 as usize, 0))
        }
    );

    ($nr:ident => fn $name:ident ( $a1:ident : $t1:ty , $a2:ident : $t2:ty,
            $a3:ident : $t3:ty, $a4:ident : $t4:ty, $a5:ident $t5:ty ) -> $t_ret:ty) => (
        #[inline(always)]
        pub unsafe fn $name($a1: $t1, $a2: $t2, $a3: $t3, $a4: $t4, $a5: $t5) -> $t_ret {
            use super::{FromUsize, arch, enums};
            FromUsize::from_usize(arch::request(0, enums::$nr as usize,
                $a1 as usize, $a2 as usize, $a3 as usize, $a4 as usize, $a5 as usize))
        }
    );
);

macro_rules! wrap_str ( ($nr:ident => fn $name:ident ( $a1:ident : &str ) -> ()) => (
    #[inline(always)]
    pub unsafe fn $name($a1: &str) {
        let c_str = CString::new($a1.as_bytes()).unwrap();
        arch::request(0, enums::$nr as usize, c_str.as_bytes_with_nul().as_ptr() as usize, 0, 0, 0, 0);
    }
));

// Wrap a function taking `(addr: *const (), len: usize)` with a function that takes
// `*const T` and uses `size_of::<T>()`
macro_rules! generic ( ($imp:ident => fn $name:ident <T>($a1:ident : *const T) -> $t_ret:ty) => (
    #[inline(always)]
    pub unsafe fn $name<T>($a1: *const T) -> $t_ret {
        use std::mem::size_of;
        $imp($a1 as *const (), size_of::<T>())
    }
));

pub mod valgrind {
    //! Client requests for the Valgrind core itself.
    //!
    //! See `/usr/include/valgrind/valgrind.h` and
    //! [section 3.1][] of the Valgrind manual.
    //!
    //! [section 3.1]: http://valgrind.org/docs/manual/manual-core-adv.html#manual-core-adv.clientreq

    use super::{arch, enums};
    use std::ffi::CString;

    wrap!(VG_USERREQ__RUNNING_ON_VALGRIND
        => fn running_on_valgrind() -> usize);

    wrap!(VG_USERREQ__COUNT_ERRORS
        => fn count_errors() -> usize);

    wrap!(VG_USERREQ__DISCARD_TRANSLATIONS
        => fn discard_translations(addr: *const (), len: usize) -> ());

    wrap_str!(VG_USERREQ__GDB_MONITOR_COMMAND
        => fn monitor_command(cmd: &str) -> ());
}

pub mod memcheck {
    //! Client requests for the Memcheck memory error
    //! detector tool.
    //!
    //! See `/usr/include/valgrind/memcheck.h` and
    //! [section 4.7][] of the Valgrind manual.
    //!
    //! [section 4.7]: http://valgrind.org/docs/manual/mc-manual.html#mc-manual.clientreqs

    wrap!(VG_USERREQ__MALLOCLIKE_BLOCK
        => fn malloclike_block(addr: *const (), size: usize, redzone: usize, is_zeroed: bool) -> ());

    wrap!(VG_USERREQ__RESIZEINPLACE_BLOCK
        => fn resizeinplace_block(addr: *const (), old_size: usize, new_size: usize, redzone: usize) -> ());

    wrap!(VG_USERREQ__FREELIKE_BLOCK
        => fn freelike_block(addr: *const (), redzone: usize) -> ());

    wrap!(VG_USERREQ__MAKE_MEM_NOACCESS
        => fn make_mem_noaccess(addr: *const (), len: usize) -> ());

    generic!(make_mem_noaccess
        => fn make_noaccess<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__MAKE_MEM_UNDEFINED
        => fn make_mem_undefined(addr: *const (), len: usize) -> ());

    generic!(make_mem_undefined
        => fn make_undefined<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__MAKE_MEM_DEFINED
        => fn make_mem_defined(addr: *const (), len: usize) -> ());

    generic!(make_mem_defined
        => fn make_defined<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__MAKE_MEM_DEFINED_IF_ADDRESSABLE
        => fn make_mem_defined_if_addressable(addr: *const (), len: usize) -> ());

    generic!(make_mem_defined_if_addressable
        => fn make_defined_if_addressable<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__CHECK_MEM_IS_ADDRESSABLE
        => fn check_mem_is_addressable(addr: *const (), len: usize) -> Option<*const ()>);

    generic!(check_mem_is_addressable
        => fn check_is_addressable<T>(obj: *const T) -> Option<*const ()>);

    wrap!(VG_USERREQ__CHECK_MEM_IS_DEFINED
        => fn check_mem_is_defined(addr: *const (), len: usize) -> Option<*const ()>);

    generic!(check_mem_is_defined
        => fn check_is_defined<T>(obj: *const T) -> Option<*const ()>);

    macro_rules! wrap_leak_check ( ($nr:ident($a1:expr, $a2:expr) => fn $name:ident () -> ()) => (
        #[inline(always)]
        pub unsafe fn $name() {
            use super::{arch, enums};
            arch::request(0, enums::$nr as usize, $a1, $a2, 0, 0, 0);
        }
    ));

    wrap_leak_check!(VG_USERREQ__DO_LEAK_CHECK(0, 0)
        => fn do_leak_check() -> ());

    wrap_leak_check!(VG_USERREQ__DO_LEAK_CHECK(0, 1)
        => fn do_added_leak_check() -> ());

    wrap_leak_check!(VG_USERREQ__DO_LEAK_CHECK(0, 2)
        => fn do_changed_leak_check() -> ());

    wrap_leak_check!(VG_USERREQ__DO_LEAK_CHECK(1, 0)
        => fn do_quick_leak_check() -> ());

    /// Result of `count_leaks` or `count_leak_blocks`, in
    /// bytes or blocks respectively.
    #[derive(Copy, Clone)]
    pub struct LeakCount {
        pub leaked: usize,
        pub dubious: usize,
        pub reachable: usize,
        pub suppressed: usize,
    }

    macro_rules! wrap_count ( ($nr:ident => fn $name:ident() -> LeakCount) => (
        #[inline(always)]
        pub unsafe fn $name() -> LeakCount {
            use super::{arch, enums};
            let mut counts = LeakCount {
                leaked: 0,
                dubious: 0,
                reachable: 0,
                suppressed: 0,
            };
            arch::request(0, enums::$nr as usize,
                (&mut counts.leaked as *mut usize) as usize,
                (&mut counts.dubious as *mut usize) as usize,
                (&mut counts.reachable as *mut usize) as usize,
                (&mut counts.suppressed as *mut usize) as usize,
                0);
            counts
        }
    ));

    wrap_count!(VG_USERREQ__COUNT_LEAKS
        => fn count_leaks() -> LeakCount);

    wrap_count!(VG_USERREQ__COUNT_LEAK_BLOCKS
        => fn count_leak_blocks() -> LeakCount);
}

pub mod callgrind {
    //! Client requests for the Callgrind profiler tool.
    //!
    //! See `/usr/include/valgrind/callgrind.h` and
    //! [section 6.5][] of the Valgrind manual.
    //!
    //! [section 6.5]: http://valgrind.org/docs/manual/cl-manual.html#cl-manual.clientrequests

    use super::{arch, enums};
    use std::ffi::CString;

    wrap!(VG_USERREQ__DUMP_STATS
        => fn dump_stats() -> ());

    wrap_str!(VG_USERREQ__DUMP_STATS_AT
        => fn dump_stats_at(pos: &str) -> ());

    wrap!(VG_USERREQ__ZERO_STATS
        => fn zero_stats() -> ());

    wrap!(VG_USERREQ__TOGGLE_COLLECT
        => fn toggle_collect() -> ());

    wrap!(VG_USERREQ__START_INSTRUMENTATION
        => fn start_instrumentation() -> ());

    wrap!(VG_USERREQ__STOP_INSTRUMENTATION
        => fn stop_instrumentation() -> ());
}

pub mod helgrind {
    //! Client requests for the Helgrind thread error
    //! detector tool.
    //!
    //! See `/usr/include/valgrind/helgrind.h` and
    //! [section 7.7][] of the Valgrind manual.
    //!
    //! [section 7.7]: http://valgrind.org/docs/manual/hg-manual.html#hg-manual.client-requests

    wrap!(VG_USERREQ__HG_CLEAN_MEMORY
        => fn clean_memory(addr: *const (), len: usize) -> ());

    generic!(clean_memory
        => fn clean<T>(obj: *const T) -> ());
}

pub mod drd {
    //! Client requests for the DRD thread error
    //! detector tool.
    //!
    //! See `/usr/include/valgrind/drd.h` and
    //! [section 8.2.5][] of the Valgrind manual.
    //!
    //! [section 8.2.5]: http://valgrind.org/docs/manual/drd-manual.html#drd-manual.clientreqs

    use super::{arch, enums};
    use libc::c_uint;
    use std::ffi::CString;

    wrap!(VG_USERREQ__DRD_CLEAN_MEMORY
        => fn clean_memory(addr: *const (), len: usize) -> ());

    generic!(clean_memory
        => fn clean<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__DRD_GET_VALGRIND_THREAD_ID
        => fn get_valgrind_threadid() -> c_uint);

    wrap!(VG_USERREQ__DRD_GET_DRD_THREAD_ID
        => fn get_drd_threadid() -> c_uint);

    wrap!(VG_USERREQ__DRD_START_SUPPRESSION
        => fn annotate_benign_race_sized(addr: *const (), len: usize) -> ());

    generic!(annotate_benign_race_sized
        => fn annotate_benign_race<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__DRD_FINISH_SUPPRESSION
        => fn stop_ignoring_sized(addr: *const (), len: usize) -> ());

    generic!(stop_ignoring_sized
        => fn stop_ignoring<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__DRD_START_TRACE_ADDR
        => fn trace_sized(addr: *const (), len: usize) -> ());

    generic!(trace_sized
        => fn trace<T>(obj: *const T) -> ());

    wrap!(VG_USERREQ__DRD_STOP_TRACE_ADDR
        => fn stop_tracing_sized(addr: *const (), len: usize) -> ());

    generic!(stop_tracing_sized
        => fn stop_tracing<T>(obj: *const T) -> ());

    macro_rules! wrap_record( ($nr:ident($n:expr) => fn $name:ident() -> ()) => (
        #[inline(always)]
        pub unsafe fn $name() {
            use super::{arch, enums};
            arch::request(0, enums::$nr as usize, $n, 0, 0, 0, 0);
        }
    ));

    wrap_record!(VG_USERREQ__DRD_RECORD_LOADS(0)
        => fn ignore_reads_begin() -> ());

    wrap_record!(VG_USERREQ__DRD_RECORD_LOADS(1)
        => fn ignore_reads_end() -> ());

    wrap_record!(VG_USERREQ__DRD_RECORD_STORES(0)
        => fn ignore_writes_begin() -> ());

    wrap_record!(VG_USERREQ__DRD_RECORD_STORES(1)
        => fn ignore_writes_end() -> ());

    wrap_str!(VG_USERREQ__DRD_SET_THREAD_NAME
        => fn annotate_thread_name(name: &str) -> ());
}
