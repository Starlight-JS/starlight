use std::{
    mem::MaybeUninit,
    ptr::{null, null_mut},
};

use libmir_sys::*;

pub struct JITState {
    pub(crate) ctx: MIR_context_t,
}

impl JITState {
    pub fn new() -> Self {
        let ctx = unsafe { MIR_init() };
        Self { ctx }
    }

    pub fn prepare(&mut self, opt_level: i32) {
        unsafe {
            c2mir_init(self.ctx);
            MIR_gen_init(self.ctx, opt_level);
        }
    }

    pub fn compile_c_code(&mut self, name: &str, source: &str) -> *const u8 {
        let cname = name.as_bytes().as_ptr() as *const i8;
        let csource = source.as_bytes().as_ptr() as *const i8;
        unsafe {
            let mut opts: c2mir_options = MaybeUninit::zeroed().assume_init();
            opts.asm_p = 1;
            return compile_c_module(self, &mut opts, csource, cname);
        }
    }

    pub fn finish(&mut self) {
        unsafe {
            MIR_gen_finish(self.ctx);
            c2mir_finish(self.ctx);
        }
    }
}

unsafe fn mir_find_function(module: MIR_module_t, name: *const i8) -> MIR_item_t {
    let mut func;
    let mut main_func = null_mut();
    func = (*module).items.head;
    while !func.is_null() {
        if (*func).item_type == MIR_item_type_t_MIR_func_item as u32
            && libc::strcmp((*(*func).u.func).name, name) == 0
        {
            main_func = func;
        }
        func = (*func).item_link.next;
    }
    main_func
}

unsafe fn mir_get_func(ctx: MIR_context_t, module: MIR_module_t, name: *const i8) -> *const u8 {
    let f = mir_find_function(module, name);
    if f.is_null() {
        panic!("Function not found");
    }
    MIR_gen(ctx, 0, f) as *const u8
}
unsafe fn compile_c_module(
    jit: &mut JITState,
    opts: *mut c2mir_options,
    input: *const i8,
    name: *const i8,
) -> *const u8 {
    let mut addr = null();
    jit.prepare(2);
    let module = mir_compile_c_module(opts, jit.ctx, input, name);
    if !module.is_null() {
        addr = mir_get_func(jit.ctx, module, name);
    }
    jit.finish();
    addr
}

struct ReadBuffer {
    current: usize,
    source: *const i8,
}

impl ReadBuffer {
    unsafe extern "C" fn getc(data: *mut libc::c_void) -> i32 {
        let buffer = &mut *data.cast::<Self>();
        let mut c = buffer.source.add(buffer.current).read() as i32;
        if c == 0 {
            c = libc::EOF as i32;
        } else {
            buffer.current += 1;
        }
        c as _
    }
}
unsafe fn mir_compile_c_module(
    opts: *mut c2mir_options,
    ctx: MIR_context_t,
    input: *const i8,
    _name: *const i8,
) -> MIR_module_t {
    let mut ret_code = 0;
    let mut module_name = [0i8; 30];
    let mut module: MIR_module_t = null_mut();
    (*opts).module_num += 1;
    libc::snprintf(
        &mut module_name[0],
        30,
        "__mod%lld__".as_bytes().as_ptr().cast(),
        (*opts).module_num,
    );
    let mut buf = ReadBuffer {
        current: 0,
        source: input,
    };
    (*opts).message_file =
        libc::fdopen(libc::STDERR_FILENO, "w+".as_bytes().as_ptr().cast()).cast();
    if c2mir_compile(
        ctx,
        opts,
        Some(ReadBuffer::getc),
        &mut buf as *mut ReadBuffer as *mut _,
        &mut module_name[0],
        libc::fdopen(libc::STDERR_FILENO, "w+".as_bytes().as_ptr().cast()).cast(),
    ) == 0
    {
        ret_code = 1;
    } else {
        module = (*MIR_get_module_list(ctx)).tail;
    }
    if ret_code == 0 && module.is_null() {
        ret_code = 1;
    }
    if ret_code == 0 && !module.is_null() {
        MIR_load_module(ctx, module);
        MIR_link(ctx, Some(MIR_set_gen_interface), None);
        return module;
    }
    null_mut()
}

pub const STARLIGHT_JIT_RUNTIME: &'static str = include_str!("jit/rt.c");
