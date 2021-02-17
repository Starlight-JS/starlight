#[cfg(target_arch = "x86_64")]
pub mod arch {
    pub type Value = u64;

    #[inline(always)]
    pub unsafe fn do_client_request(default: Value, args: &[Value; 6]) -> Value {
        let result;
        llvm_asm!("rolq $$3,  %rdi ; rolq $$13, %rdi
            rolq $$61, %rdi ; rolq $$51, %rdi
            xchgq %rbx, %rbx"
          : "={rdx}" (result)
          : "{rax}" (args.as_ptr())
            "{rdx}" (default)
          : "cc", "memory"
          : "volatile");
        result
    }
}
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum ClientRequest {
    RunningOnValgrind = 0x1001,
    DiscardTranslations = 0x1002,
    ClientCall0 = 0x1101,
    ClientCall1 = 0x1102,
    ClientCall2 = 0x1103,
    ClientCall3 = 0x1104,
    CountErrors = 0x1201,
    GdbMonitorCommand = 0x1202,
    MallocLikeBlock = 0x1301,
    ResizeInPlaceBlock = 0x130b,
    FreeLikeBlock = 0x1302,
    CreateMemPool = 0x1303,
    DestroyMemPool = 0x1304,
    MemPoolAlloc = 0x1305,
    MemPoolFree = 0x1306,
    MemPoolTrim = 0x1307,
    MoveMemPool = 0x1308,
    MemPoolChange = 0x1309,
    MemPoolExists = 0x130a,
    Printf = 0x1401,
    PrintfBacktrace = 0x1402,
    PrintfVaListByRef = 0x1403,
    PrintfBacktraceVaListByRef = 0x1404,
    StackRegister = 0x1501,
    StackDeregister = 0x1502,
    StackChange = 0x1503,
    LoadPdbDebugInfo = 0x1601,
    MapIpToSourceLoc = 0x1701,
    ChangeErrDisablement = 0x1801,
    VexInitForIri = 0x1901,
}

pub use arch::do_client_request;
use arch::Value;

pub fn freelike(addr: usize) -> Value {
    unsafe {
        do_client_request(
            0,
            &[
                ClientRequest::FreeLikeBlock as Value,
                addr as Value,
                0,
                0,
                0,
                0,
            ],
        )
    }
}

pub fn malloc_like(addr: usize, size: usize) -> Value {
    unsafe {
        do_client_request(
            0,
            &[
                ClientRequest::MallocLikeBlock as _,
                addr as _,
                size as _,
                0,
                1,
                0,
            ],
        )
    }
}
