use crate::{prelude::*, vm::code_block::CodeBlock, vm::value::*};
use libmir_sys::*;
use once_cell::sync::Lazy;
use std::{
    any::TypeId,
    cell::RefCell,
    fmt::Write,
    mem::transmute,
    mem::MaybeUninit,
    ops::Try,
    ptr::{null, null_mut},
};

#[repr(C)]
pub struct JITResult {
    pub value: JsValue,
    pub is_err: u8,
}

impl std::ops::Try for JITResult {
    type Error = JsValue;
    type Ok = JsValue;
    fn from_error(v: Self::Error) -> Self {
        Self {
            value: v,
            is_err: 1,
        }
    }
    fn from_ok(v: Self::Ok) -> Self {
        Self {
            value: v,
            is_err: 0,
        }
    }

    fn into_result(self) -> Result<Self::Ok, Self::Error> {
        if self.is_err == 0 {
            Ok(self.value)
        } else {
            Err(self.value)
        }
    }
}

impl Into<Result<JsValue, JsValue>> for JITResult {
    fn into(self) -> Result<JsValue, JsValue> {
        self.into_result()
    }
}
impl Into<JITResult> for Result<JsValue, JsValue> {
    fn into(self) -> JITResult {
        match self {
            Ok(x) => JITResult::from_ok(x),
            Err(x) => JITResult::from_error(x),
        }
    }
}

extern "C" fn jsval_to_num_slow(rt: &mut Runtime, val: JsValue) -> JITResult {
    val.to_number(rt)
        .map(|x| JsValue::encode_f64_value(x))
        .into()
}
extern "C" fn op_add_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let lhs = lhs.to_primitive(rt, JsHint::None)?;
    let rhs = rhs.to_primitive(rt, JsHint::None)?;
    if lhs.is_int32() && rhs.is_int32() {
        if let Some(res) = lhs.get_int32().checked_add(rhs.get_int32()) {
            return Ok(JsValue::encode_int32(res)).into();
        }
    }
    if lhs.is_number() && rhs.is_number() {
        return Ok(JsValue::encode_f64_value(
            lhs.get_number() + rhs.get_number(),
        ))
        .into();
    }
    if lhs.is_jsstring() || rhs.is_jsstring() {
        #[inline(never)]
        fn concat(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> Result<JsValue, JsValue> {
            let lhs = lhs.to_string(rt)?;
            let rhs = rhs.to_string(rt)?;
            let string = format!("{}{}", lhs, rhs);
            Ok(JsValue::encode_object_value(JsString::new(rt, string)))
        }

        let result = concat(rt, lhs, rhs)?;
        return Ok(result).into();
    } else {
        let lhs = lhs.to_number(rt)?;
        let rhs = rhs.to_number(rt)?;
        return Ok(JsValue::new(lhs + rhs)).into();
    }
}

pub extern "C" fn op_sub_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let lhs = lhs.to_number(rt)?;
    let rhs = rhs.to_number(rt)?;
    Ok(JsValue::new(lhs - rhs)).into()
}

pub extern "C" fn op_div_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let lhs = lhs.to_number(rt)?;
    let rhs = rhs.to_number(rt)?;
    Ok(JsValue::new(lhs / rhs)).into()
}

pub extern "C" fn op_mul_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let lhs = lhs.to_number(rt)?;
    let rhs = rhs.to_number(rt)?;
    Ok(JsValue::new(lhs * rhs)).into()
}

pub extern "C" fn op_rem_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let lhs = lhs.to_number(rt)?;
    let rhs = rhs.to_number(rt)?;
    Ok(JsValue::new(lhs % rhs)).into()
}

pub extern "C" fn op_shl_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let left = lhs.to_int32(rt)?;
    let right = rhs.to_uint32(rt)?;
    Ok(JsValue::new((left << (right & 0x1f)) as f64)).into()
}

pub extern "C" fn op_shr_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let left = lhs.to_int32(rt)?;
    let right = rhs.to_uint32(rt)?;
    Ok(JsValue::new((left >> (right & 0x1f)) as f64)).into()
}

pub extern "C" fn op_ushr_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    let left = lhs.to_uint32(rt)?;
    let right = rhs.to_uint32(rt)?;
    Ok(JsValue::new((left >> (right & 0x1f)) as f64)).into()
}

pub extern "C" fn op_less_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    Ok(JsValue::new(lhs.compare(rhs, true, rt)? == CMP_TRUE)).into()
}
pub extern "C" fn op_lesseq_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    Ok(JsValue::new(rhs.compare(lhs, false, rt)? == CMP_FALSE)).into()
}

pub extern "C" fn op_greater_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    Ok(JsValue::new(rhs.compare(lhs, false, rt)? == CMP_TRUE)).into()
}
pub extern "C" fn op_greatereq_slow(rt: &mut Runtime, lhs: JsValue, rhs: JsValue) -> JITResult {
    Ok(JsValue::new(lhs.compare(rhs, true, rt)? == CMP_FALSE)).into()
}

/// Used by import resolver when compiling C module in C2MIR.
pub static STARLIGHT_SYMBOLS: Lazy<&'static [(&'static str, usize)]> = Lazy::new(|| {
    Box::leak(Box::new([
        ("get_jscell_type_id", get_jscell_type_id as usize),
        ("jsval_to_number_slow", jsval_to_num_slow as usize),
        ("op_add_slow", op_add_slow as usize),
        ("op_sub_slow", op_sub_slow as _),
        ("op_div_slow", op_div_slow as _),
        ("op_mul_slow", op_mul_slow as _),
        ("op_rem_slow", op_rem_slow as _),
        ("op_shl_slow", op_shl_slow as _),
        ("op_shr_slow", op_shr_slow as _),
        ("op_ushr_slow", op_ushr_slow as _),
        ("op_less_slow", op_less_slow as _),
        ("op_lesseq_slow", op_lesseq_slow as _),
        ("op_greater_slow", op_greater_slow as _),
        ("op_greatereq_slow", op_greatereq_slow as _),
    ]))
});

extern "C" fn import_resolver(name: *const i8) -> *mut libc::c_void {
    unsafe {
        let name = std::ffi::CStr::from_ptr(name);
        for (fname, func) in STARLIGHT_SYMBOLS.iter() {
            if *fname == name.to_str().unwrap() {
                return (*func) as _;
            }
        }
        null_mut()
    }
}

pub struct JITState {
    pub(crate) ctx: MIR_context_t,
    pub(crate) tmp_var_id: u32,
}

impl JITState {
    pub fn new() -> Self {
        let ctx = unsafe { MIR_init() };
        Self { ctx, tmp_var_id: 0 }
    }
    pub fn new_temp(&mut self) -> u32 {
        self.tmp_var_id += 1;
        self.tmp_var_id - 1
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

    fn compile_internal(&mut self, code_block: GcPointer<CodeBlock>) {}
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
        MIR_link(ctx, Some(MIR_set_gen_interface), Some(import_resolver));
        return module;
    }
    null_mut()
}

pub const STARLIGHT_JIT_RUNTIME: &'static str = include_str!("jit/rt.c");
/// Minimal string size that is preallocated for JIT compiler. When JIT finishes
/// this string will be saved in thread-local state and its capacity will be shrinked
/// down to JIT_STRING_CAPACITY again.
pub const JIT_STRING_CAPACITY: usize = 8 * 1024;

pub static JIT_RUNTIME: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| unsafe {
    let obj_type_id: u64 = transmute(TypeId::of::<JsObject>());
    let str_type_id: u64 = transmute(TypeId::of::<JsString>());
    let mut source = String::with_capacity(JIT_STRING_CAPACITY + 32);
    writeln!(
        &mut source,
        "#define JSOBJECT_TYPEID {}\n #define JSSTRING_TYPEID {}\n{}",
        obj_type_id, str_type_id, STARLIGHT_JIT_RUNTIME
    )
    .unwrap();
    source
});

thread_local! {
    static JIT_STORAGE: RefCell<Option<String>> = RefCell::new(Some(String::with_capacity(8*1024)));
}

pub(super) fn jit_take_storage() -> String {
    JIT_STORAGE.with(|x| x.borrow_mut().take().unwrap())
}
pub(super) fn jit_put_storage(storage: String) {
    JIT_STORAGE.with(|x| *x.borrow_mut() = Some(storage));
}
