#![allow(dead_code)]
use crate::{
    define_jsclass_with_symbol,
    gc::{
        cell::GcPointer,
        snapshot::{deserializer::Deserializer, serializer::SnapshotSerializer},
    },
    prelude::*,
};
use libffi::low::{
    call as ffi_call, ffi_abi_FFI_DEFAULT_ABI as ABI, ffi_cif, ffi_type, prep_cif, types, CodePtr,
    Error as FFIError,
};
use std::ffi::{CStr, OsStr};
use std::fmt::{Debug, Display};
use std::mem;
use std::mem::ManuallyDrop;
use std::os::raw::{
    c_char, c_double, c_float, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void,
};
use std::ptr;
use std::{convert::Into, mem::size_of};

/// A pointer to an FFI type.
pub type TypePointer = *mut ffi_type;

/// A raw C pointer.
pub type RawPointer = *mut c_void;

pub fn initialize_ffi(rt: &mut Runtime) {
    rt.gc().defer();
    let structure =
        Structure::new_indexed(rt, Some(rt.global_data.object_prototype.unwrap()), false);
    let mut init = || -> Result<(), JsValue> {
        let mut proto = JsObject::new(rt, &structure, JsObject::get_class(), ObjectTag::Ordinary);
        let func = JsNativeFunction::new(rt, "open".intern(), ffi_library_open, 1);
        proto.define_own_property(
            rt,
            "open".intern(),
            &*DataDescriptor::new(JsValue::new(func), NONE),
            false,
        )?;
        rt.global_object().define_own_property(
            rt,
            "FFI".intern(),
            &*DataDescriptor::new(JsValue::new(proto), E),
            false,
        )?;
        let func_s =
            Structure::new_indexed(rt, Some(rt.global_data.object_prototype.unwrap()), false);
        let mut fproto = JsObject::new(rt, &func_s, JsObject::get_class(), ObjectTag::Ordinary);
        let func = JsNativeFunction::new(rt, "attach".intern(), ffi_function_attach, 1);
        fproto.define_own_property(
            rt,
            "attach".intern(),
            &*DataDescriptor::new(JsValue::new(func), E),
            false,
        )?;
        let func = JsNativeFunction::new(rt, "call".intern(), ffi_function_call, 1);
        fproto.define_own_property(
            rt,
            "call".intern(),
            &*DataDescriptor::new(JsValue::new(func), E),
            false,
        )?;

        rt.global_object().define_own_property(
            rt,
            "CFunction".intern(),
            &*DataDescriptor::new(JsValue::new(fproto), E),
            false,
        )?;

        rt.eval(
            false,
            r#"
globalThis.FFI.void = 0
globalThis.FFI.pointer = 1
globalThis.FFI.f64 = 2
globalThis.FFI.f32 = 3
globalThis.FFI.i8 = 4
globalThis.FFI.i16 = 5
globalThis.FFI.i32 = 6
globalThis.FFI.i64 = 7
globalThis.FFI.u8 = 8
globalThis.FFI.u16 = 9
globalThis.FFI.u32 = 10
globalThis.FFI.u64 = 11
globalThis.FFI.cstring = 12

FFI.toFFItype = function(val) {
    let ty = typeof val;
    if (ty == "string") {
        return FFI.cstring;
    } else if (ty == "number") {
        return FFI.f64;
    } else if (ty == "undefined") {
        return FFI.void;
    } else {
        throw "Todo";
    }
}
// allows to create CFunction with variadic arguments. 
CFunction.create = function cnew(library, name, args, ret, variadic) {
    if (!variadic) {
        return CFunction.attach(library, name, args, ret);
    } else {
        let real_args = args;
        return {
            call: function call(...args) {
                let vargs = []
                let types = []
                for (let i = 0; i < real_args.length; i += 1) {
                    vargs.push(args[i]);
                    types.push(real_args[i]);
                }
                for (let i = real_args.length; i < args.length; i += 1) {
                    vargs.push(args[i]);
                    types.push(FFI.toFFItype(args[i]));
                }

                let cfunc = CFunction.attach(library, name, types, ret);

                return cfunc.call(vargs);
            }
        }
    }
}
        "#,
        )?;
        Ok(())
    };

    match init() {
        Ok(_) => (),
        Err(_) => {
            unreachable!()
        }
    }
    rt.gc().undefer();
}
/// A wrapper around a C pointer.
#[derive(Clone, Copy)]
pub struct Pointer {
    inner: RawPointer,
}

unsafe impl Send for Pointer {}

/// The numeric identifier of the C `void` type.
const TYPE_VOID: i64 = 0;

/// The numeric identifier of the C `void*` type.
const TYPE_POINTER: i64 = 1;

/// The numeric identifier of the C `double` type.
const TYPE_DOUBLE: i64 = 2;

/// The numeric identifier of the C `float` type.
const TYPE_FLOAT: i64 = 3;

/// The numeric identifier of the C `signed char` type.
const TYPE_I8: i64 = 4;

/// The numeric identifier of the C `short` type.
const TYPE_I16: i64 = 5;

/// The numeric identifier of the C `int` type.
const TYPE_I32: i64 = 6;

/// The numeric identifier of the C `long` type.
const TYPE_I64: i64 = 7;

/// The numeric identifier of the C `unsigned char` type.
const TYPE_U8: i64 = 8;

/// The numeric identifier of the C `unsigned short` type.
const TYPE_U16: i64 = 9;

/// The numeric identifier of the C `unsigned int` type.
const TYPE_U32: i64 = 10;

/// The numeric identifier of the C `unsigned long` type.
const TYPE_U64: i64 = 11;

/// The numeric identifier for the C `const char*` type.
const TYPE_STRING: i64 = 12;

/// The numeric identifier for a C `const char*` type that should be read into a
/// byte array..
const TYPE_BYTE_ARRAY: i64 = 13;

/// The numeric identifier of the C `size_t` type.
const TYPE_SIZE_T: i64 = 14;
pub struct FFIFunction {
    /// The pointer to the function to call.
    pointer: Pointer,

    /// The CIF (Call Interface) to use for this function.
    cif: ffi_cif,

    /// The argument types of the function.
    arguments: Vec<TypePointer>,

    /// The return type of the function.
    return_type: TypePointer,
}

extern "C" fn drop_ffi_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<FFIFunction>()) }
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer, _: &mut Runtime) {
    unreachable!("Cannot deserialize FFI function");
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    unreachable!("Cannot serialize FFI function");
}
extern "C" fn fsz() -> usize {
    size_of::<FFIFunction>()
}
impl FFIFunction {
    define_jsclass_with_symbol!(
        JsObject,
        FFIFunction,
        Object,
        Some(drop_ffi_fn),
        None,
        Some(deser),
        Some(ser),
        Some(fsz)
    );
}

pub struct FFILibrary {
    library: Option<libloading::Library>,
}

extern "C" fn drop_ffi_lib(obj: &mut JsObject) {
    obj.data::<FFILibrary>().close();
    unsafe { ManuallyDrop::drop(obj.data::<FFILibrary>()) }
}

extern "C" fn deser_lib(_: &mut JsObject, _: &mut Deserializer, _: &mut Runtime) {
    unreachable!("Cannot deserialize FFI library");
}

extern "C" fn ser_lib(_: &JsObject, _: &mut SnapshotSerializer) {
    unreachable!("Cannot serialize FFI library");
}

extern "C" fn sz() -> usize {
    size_of::<FFILibrary>()
}

impl FFILibrary {
    define_jsclass_with_symbol!(
        JsObject,
        FFILibrary,
        Object,
        Some(drop_ffi_lib),
        None,
        Some(deser_lib),
        Some(ser_lib),
        Some(sz)
    );
}

/// Returns a pointer to a statically allocated FFI type.
macro_rules! ffi_type {
    ($name: ident) => {
        &types::$name as *const ffi_type as *mut ffi_type
    };
}

/// Converts a &T to a *mut c_void pointer.
macro_rules! raw_pointer {
    ($value: expr) => {
        $value as *mut _ as RawPointer
    };
}

/// Generates a "match" that can be used for pattern matching a pointer to an
/// FFI type.
///
/// For example, this macro call:
///
///     match_ffi_type!(
///       some_variable,
///       pointer => { 10 }
///       void => { 20 }
///     );
///
/// Would compile into:
///
///     match some_variable {
///         t if t == ffi_type!(pointer) => { 10 }
///         t if t == ffi_type!(void) => { 20 }
///         _ => unreachable!()
///     }
///
/// Just like a regular `match`, `match_ffi_type!` supports OR conditions:
///
///     match_ffi_type!(
///       some_variable,
///       pointer => { 10 }
///       void => { 20 }
///       sint8 | sint16 | sint32 | sint64 => { 30 }
///     );
///
/// This would compile into the following:
///
///     match some_variable {
///         t if t == ffi_type!(pointer) => { 10 }
///         t if t == ffi_type!(void) => { 20 }
///         t if t == ffi_type!(sint8) => { 30 }
///         t if t == ffi_type!(sint16) => { 30 }
///         t if t == ffi_type!(sint32) => { 30 }
///         t if t == ffi_type!(sint64) => { 30 }
///         _ => unreachable!()
///     }
macro_rules! match_ffi_type {
    (
        $pointer: expr,

        $(
            $($type: ident)|+ => $body: expr
        )+
    ) => {
        match $pointer {
            $(
                $(
                    t if t == ffi_type!($type) => { $body }
                )+
            )+
            _ => unreachable!()
        }
    }
}

macro_rules! ffi_type_error {
    ($rt: expr,$type: expr) => {
        return Err(JsValue::new(JsString::new(
            $rt,
            format!("Invalid FFI type: {}", $type),
        )));
    };
}

/// Returns the size of a type ID.
///
/// The size of the type is returned as a tagged integer.
pub fn type_size(rt: &mut Runtime, id: i64) -> Result<JsValue, JsValue> {
    let size = unsafe {
        match id {
            TYPE_VOID => types::void.size,
            TYPE_POINTER | TYPE_STRING | TYPE_BYTE_ARRAY => types::pointer.size,
            TYPE_DOUBLE => types::double.size,
            TYPE_FLOAT => types::float.size,
            TYPE_I8 => types::sint8.size,
            TYPE_I16 => types::sint16.size,
            TYPE_I32 => types::sint32.size,
            TYPE_I64 => types::sint64.size,
            TYPE_U8 => types::uint8.size,
            TYPE_U16 => types::uint16.size,
            TYPE_U32 => types::uint32.size,
            TYPE_U64 => types::uint64.size,
            TYPE_SIZE_T => mem::size_of::<usize>(),
            _ => ffi_type_error!(rt, id),
        }
    };

    Ok(JsValue::new(size as u32))
}

/// Returns the alignment of a type ID.
///
/// The alignment of the type is returned as a tagged integer.
pub fn type_alignment(rt: &mut Runtime, id: i64) -> Result<JsValue, JsValue> {
    let size = unsafe {
        match id {
            TYPE_VOID => types::void.alignment,
            TYPE_POINTER | TYPE_STRING | TYPE_BYTE_ARRAY => types::pointer.alignment,
            TYPE_DOUBLE => types::double.alignment,
            TYPE_FLOAT => types::float.alignment,
            TYPE_I8 => types::sint8.alignment,
            TYPE_I16 => types::sint16.alignment,
            TYPE_I32 => types::sint32.alignment,
            TYPE_I64 => types::sint64.alignment,
            TYPE_U8 => types::uint8.alignment,
            TYPE_U16 => types::uint16.alignment,
            TYPE_U32 => types::uint32.alignment,
            TYPE_U64 => types::uint64.alignment,
            TYPE_SIZE_T => mem::align_of::<usize>() as u16,
            _ => ffi_type_error!(rt, id),
        }
    };

    Ok(JsValue::new(size as u32))
}

/// A value of some sort to be passed to a C function.
pub enum Argument {
    Pointer(RawPointer),
    Void,
    F32(f32),
    F64(f64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
}

impl Argument {
    unsafe fn wrap(
        ffi_type: *mut ffi_type,
        val: JsValue,
        rt: &mut Runtime,
    ) -> Result<Argument, JsValue> {
        match_ffi_type!(
            ffi_type,
            pointer => {
                if val.is_number() {
                    let n = val.get_number();
                    if n as i32 as f64 == n
                    {
                        return Ok(Argument::I32(n as i32));
                    } else {
                        return Ok(Argument::F64(n));
                    }

                } else if val.is_jsstring() {
                     return Ok(Argument::Pointer(val.get_jsstring().as_str().as_ptr() as *mut _));
                } else {
                    let val_str = val.to_string(rt);
                    let val_str = if let Ok(val_str) = val_str {
                        val_str
                    } else {
                        "<unknown>".to_owned()
                    };
                    return Err(JsValue::new(JsString::new(rt,format!("Cannot passs value '{}' as pointer",val_str))));
                }
            }
            void => return Ok(Argument::Void)
            float => return Ok(Argument::F32(val.to_number(rt)? as f32))
            double => return Ok(Argument::F64(val.to_number(rt)?))
            sint8 => return Ok(Argument::I8(val.to_int32(rt)? as _))
            sint16 => return Ok(Argument::I16(val.to_int32(rt)? as _))
            sint32 => return Ok(Argument::I32(val.to_int32(rt)? as _))
            sint64 => return Ok(Argument::I64(val.to_int32(rt)? as _))
            uint8 => return Ok(Argument::U8(val.to_uint32(rt)? as _))
            uint16 => return Ok(Argument::U16(val.to_uint32(rt)? as _))
            uint32 => return Ok(Argument::U32(val.to_uint32(rt)? as _))
            uint64 => return Ok(Argument::U64(val.to_uint32(rt)? as _))

        );
    }

    /// Returns a C pointer to the wrapped value.
    fn as_c_pointer(&mut self) -> RawPointer {
        match self {
            Argument::Pointer(ref mut val) => {
                // When passing a pointer we shouldn't pass the pointer
                // directly, instead we want a pointer to the pointer to pass to
                // the underlying C function.
                val as *mut RawPointer as RawPointer
            }
            Argument::Void => ptr::null_mut() as RawPointer,
            Argument::F32(ref mut val) => raw_pointer!(val),
            Argument::F64(ref mut val) => raw_pointer!(val),
            Argument::I8(ref mut val) => raw_pointer!(val),
            Argument::I16(ref mut val) => raw_pointer!(val),
            Argument::I32(ref mut val) => raw_pointer!(val),
            Argument::I64(ref mut val) => raw_pointer!(val),
            Argument::U8(ref mut val) => raw_pointer!(val),
            Argument::U16(ref mut val) => raw_pointer!(val),
            Argument::U32(ref mut val) => raw_pointer!(val),
            Argument::U64(ref mut val) => raw_pointer!(val),
        }
    }
}
/// Returns an FFI type for an integer pointer.
unsafe fn ffi_type_for(pointer: JsValue, rt: &mut Runtime) -> Result<TypePointer, JsValue> {
    let int = pointer.to_int32(rt)?;
    let typ = match int as i64 {
        TYPE_VOID => ffi_type!(void),
        TYPE_POINTER | TYPE_STRING | TYPE_BYTE_ARRAY => ffi_type!(pointer),
        TYPE_DOUBLE => ffi_type!(double),
        TYPE_FLOAT => ffi_type!(float),
        TYPE_I8 => ffi_type!(sint8),
        TYPE_I16 => ffi_type!(sint16),
        TYPE_I32 => ffi_type!(sint32),
        TYPE_I64 => ffi_type!(sint64),
        TYPE_U8 => ffi_type!(uint8),
        TYPE_U16 => ffi_type!(uint16),
        TYPE_U32 => ffi_type!(uint32),
        TYPE_U64 => ffi_type!(uint64),
        TYPE_SIZE_T => {
            match mem::size_of::<usize>() {
                64 => ffi_type!(uint64),
                32 => ffi_type!(uint32),
                8 => ffi_type!(uint8),

                // The C spec states that `size_t` is at least 16 bits, so we
                // can use this as the default.
                _ => ffi_type!(uint16),
            }
        }
        _ => ffi_type_error!(rt, int),
    };

    Ok(typ as TypePointer)
}

impl FFILibrary {
    /// Opens a library using one or more possible names, stored as pointers to
    /// heap allocated objects.
    pub fn from_pointers(rt: &mut Runtime, search_for: &[JsValue]) -> Result<FFILibrary, JsValue> {
        let mut names = Vec::with_capacity(search_for.len());

        for name in search_for {
            names.push(name.to_string(rt)?);
        }

        Self::open(&names).map_err(|err| JsValue::new(JsString::new(rt, err)))
    }

    /// Opens a library using one or more possible names.
    pub fn open<P: AsRef<OsStr> + Debug + Display>(search_for: &[P]) -> Result<FFILibrary, String> {
        let mut errors = Vec::new();

        unsafe {
            for name in search_for {
                match libloading::Library::new(name).map(|raw| FFILibrary { library: Some(raw) }) {
                    Ok(library) => return Ok(library),
                    Err(err) => {
                        errors.push(format!("\n{}: {}", name, err));
                    }
                }
            }

            let mut error_message = "Unable to open the supplied libraries:\n".to_string();

            for error in errors {
                error_message.push_str(&error);
            }

            Err(error_message)
        }
    }

    /// Obtains a pointer to a symbol.
    ///
    /// This method is unsafe because the pointer could be of any type, thus it
    /// is up to the caller to make sure the result is used appropriately.
    pub unsafe fn get(&self, name: &str) -> Result<Pointer, String> {
        let inner = if let Some(ref inner) = self.library {
            inner
        } else {
            return Err("The library has been closed".to_string());
        };

        inner
            .get(name.as_bytes())
            .map(|sym: libloading::Symbol<RawPointer>| Pointer::new(*sym))
            .map_err(|err| err.to_string())
    }

    pub fn close(&mut self) {
        drop(self.library.take());
    }
}

impl Pointer {
    pub fn new(inner: RawPointer) -> Self {
        Pointer { inner }
    }

    /// Returns the address of this pointer.
    pub fn address(self) -> usize {
        self.inner as usize
    }

    /// Reads the value of this pointer into a particular type, based on the
    /// integer specified in `kind`.
    pub unsafe fn read_as(self, rt: &mut Runtime, kind: JsValue) -> Result<JsValue, JsValue> {
        let int = kind.to_int32(rt)? as i64;
        let pointer = match int {
            TYPE_POINTER => {
                todo!()
            }
            TYPE_STRING => {
                let string = self.read_cstr().to_string_lossy().into_owned();

                JsValue::new(JsString::new(rt, string))
            }
            TYPE_BYTE_ARRAY => {
                todo!()
            }
            TYPE_DOUBLE => self.read_float::<c_double>(),
            TYPE_FLOAT => self.read_float::<c_float>(),
            TYPE_I8 => self.read_signed_integer::<c_char>(),
            TYPE_I16 => self.read_signed_integer::<c_short>(),
            TYPE_I32 => self.read_signed_integer::<c_int>(),
            TYPE_I64 => self.read_signed_integer::<c_long>(),
            TYPE_U8 => self.read_unsigned_integer::<c_uchar>(),
            TYPE_U16 => self.read_unsigned_integer::<c_ushort>(),
            TYPE_U32 => self.read_unsigned_integer::<c_uint>(),
            TYPE_U64 => self.read_unsigned_integer::<c_ulong>(),
            TYPE_SIZE_T => match mem::size_of::<usize>() {
                64 => self.read_unsigned_integer::<c_ulong>(),
                32 => self.read_unsigned_integer::<c_uint>(),
                16 => self.read_unsigned_integer::<c_ushort>(),
                8 => self.read_unsigned_integer::<c_uchar>(),
                _ => unreachable!(),
            },
            _ => ffi_type_error!(rt, int),
        };

        Ok(pointer)
    }

    /// Writes a value to the underlying pointer.
    pub unsafe fn write_as(
        self,
        rt: &mut Runtime,
        kind: JsValue,
        value: JsValue,
    ) -> Result<(), JsValue> {
        let int = kind.to_int32(rt)? as i64;

        match int {
            TYPE_STRING => {
                let string = value.to_string(rt)?;

                ptr::copy(
                    string.as_ptr() as *mut c_char,
                    self.inner as *mut c_char,
                    string.len(),
                );
            }
            TYPE_BYTE_ARRAY => {
                todo!("byte array");
            }
            TYPE_POINTER => todo!(),
            TYPE_DOUBLE => self.write(value.to_number(rt)?),
            TYPE_FLOAT => self.write(value.to_number(rt)? as f32),
            TYPE_I8 => self.write(value.to_int32(rt)? as i8),
            TYPE_I16 => self.write(value.to_int32(rt)? as i16),
            TYPE_I32 => self.write(value.to_int32(rt)?),
            TYPE_I64 => self.write(value.to_int32(rt)? as i64),
            TYPE_U8 => self.write(value.to_uint32(rt)? as u8),
            TYPE_U16 => self.write(value.to_uint32(rt)? as u16),
            TYPE_U32 => self.write(value.to_uint32(rt)?),
            TYPE_U64 => self.write(value.to_uint32(rt)? as u64),
            TYPE_SIZE_T => self.write(value.to_uint32(rt)? as usize),
            _ => ffi_type_error!(rt, int),
        };

        Ok(())
    }

    /// Returns a new Pointer, optionally starting at the given offset.
    ///
    /// The `offset` argument is the offset in _bytes_, not the number of
    /// elements (unlike Rust's `pointer::offset`).
    pub fn with_offset(self, offset_bytes: usize) -> Self {
        let inner = (self.inner as usize + offset_bytes) as RawPointer;

        Pointer::new(inner)
    }

    /// Returns the underlying C pointer.
    fn as_c_pointer(self) -> RawPointer {
        self.inner
    }

    unsafe fn read<R>(self) -> R {
        ptr::read(self.inner as *mut R)
    }

    unsafe fn write<T>(self, value: T) {
        ptr::write(self.inner as *mut T, value);
    }

    unsafe fn read_signed_integer<T: Into<i64>>(self) -> JsValue {
        JsValue::new(self.read::<T>().into())
    }

    unsafe fn read_unsigned_integer<T: Into<u64>>(self) -> JsValue {
        JsValue::new(self.read::<T>().into())
    }

    unsafe fn read_float<T: Into<f64>>(self) -> JsValue {
        JsValue::new(self.read::<T>().into())
    }

    unsafe fn read_cstr<'a>(self) -> &'a CStr {
        CStr::from_ptr(self.inner as *mut c_char)
    }
}

impl FFIFunction {
    /// Creates a new function using object pointers.
    pub unsafe fn attach(
        rt: &mut Runtime,
        library: &FFILibrary,
        name: &str,
        arguments: &[JsValue],
        return_type: JsValue,
    ) -> Result<GcPointer<JsObject>, JsValue> {
        let func_ptr = library
            .get(name)
            .map_err(|x| JsValue::new(JsString::new(rt, x)))?;
        let ffi_rtype = ffi_type_for(return_type, rt)?;
        let mut ffi_arg_types = Vec::with_capacity(arguments.len());

        for ptr in arguments {
            ffi_arg_types.push(ffi_type_for(*ptr, rt)?);
        }

        Self::create(rt, func_ptr, ffi_arg_types, ffi_rtype).map_err(|e| e.into())
    }

    /// Creates a new prepared function.
    unsafe fn create(
        rt: &mut Runtime,
        pointer: Pointer,
        arguments: Vec<TypePointer>,
        return_type: TypePointer,
    ) -> Result<GcPointer<JsObject>, JsValue> {
        let mut func = FFIFunction {
            pointer,
            cif: Default::default(),
            arguments,
            return_type,
        };

        let result = prep_cif(
            &mut func.cif,
            ABI,
            func.arguments.len(),
            func.return_type,
            func.arguments.as_mut_ptr(),
        );

        let f = result
            .map(|_| func)
            .map_err(|err| match err {
                FFIError::Typedef => {
                    "The type representation is invalid or unsupported".to_string()
                }
                FFIError::Abi => "The ABI is invalid or unsupported".to_string(),
            })
            .map_err(|x| JsValue::new(JsString::new(rt, x)))?;

        let ffi_object = rt.global_object().get(rt, "CFunction".intern())?;
        let structure = Structure::new_indexed(rt, Some(ffi_object.get_jsobject()), false);
        let mut object = JsObject::new(
            rt,
            &structure,
            FFIFunction::get_class(),
            ObjectTag::Ordinary,
        );
        unsafe {
            (object.data::<FFIFunction>() as *mut ManuallyDrop<Self> as *mut Self).write(f);
        }
        Ok(object)
    }

    /// Calls the function with the given arguments.
    pub unsafe fn call(&self, rt: &mut Runtime, arg_ptrs: &[JsValue]) -> Result<JsValue, JsValue> {
        if arg_ptrs.len() != self.arguments.len() {
            return Err(JsValue::new(JsString::new(
                rt,
                format!(
                    "Invalid number of arguments, expected {} but got {}",
                    self.arguments.len(),
                    arg_ptrs.len()
                ),
            )));
        }

        let mut arguments = Vec::with_capacity(arg_ptrs.len());

        for (index, arg) in arg_ptrs.iter().enumerate() {
            arguments.push(Argument::wrap(self.arguments[index], *arg, rt)?);
        }

        // libffi expects an array of _pointers_ to the arguments to pass,
        // instead of an array containing the arguments directly. The pointers
        // and the values they point to must outlive the FFI call, otherwise we
        // may end up passing pointers to invalid memory.
        let mut argument_pointers: Vec<RawPointer> =
            arguments.iter_mut().map(Argument::as_c_pointer).collect();

        // libffi requires a mutable pointer to the CIF, but "self" is immutable
        // since we never actually modify the current function. To work around
        // this we manually cast to a mutable pointer.
        let cif_ptr = &self.cif as *const _ as *mut _;
        let fun_ptr = CodePtr::from_ptr(self.pointer.inner);
        let args_ptr = argument_pointers.as_mut_ptr();

        // Instead of reading the result into some kind of generic pointer (*mut
        // c_void for example) and trying to cast that to the right type, we'll
        // immediately read the call's return value into the right type. This
        // requires a bit more code, but is much less unsafe than trying to cast
        // types from X to Y without knowing if this even works reliably.
        let pointer = match_ffi_type!(
            self.return_type,
            pointer => {
                let result: RawPointer = ffi_call(cif_ptr, fun_ptr, args_ptr);

                todo!()
            }
            void => {
                ffi_call::<c_void>(cif_ptr, fun_ptr, args_ptr);

                JsValue::encode_undefined_value()
            }
            double | float => {
                let result: c_double = ffi_call(cif_ptr, fun_ptr, args_ptr);

                JsValue::new(result as f64)
                            }
            sint8 | sint16 | sint32 | sint64 => {
                let result: c_long = ffi_call(cif_ptr, fun_ptr, args_ptr);

               JsValue::new(result as i32)
            }
            uint8 | uint16 | uint32 | uint64 => {
                let result: c_ulong = ffi_call(cif_ptr, fun_ptr, args_ptr);

                JsValue::new(result as u32)
            }
        );

        Ok(pointer)
    }
}

pub fn ffi_library_open(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let names = args.at(0);
    if !names.is_jsobject() {
        let msg = JsString::new(
            rt,
            "library_open requires array-like object of library names",
        );
        return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
    }
    let stack = rt.shadowstack();

    root!(rnames = stack, vec![]);
    root!(names = stack, names.get_jsobject());
    let len = super::get_length(rt, &mut names)?;

    for i in 0..len {
        rnames.push(names.get(rt, Symbol::Index(i))?);
    }

    let lib = FFILibrary::from_pointers(rt, &rnames)?;
    let proto = rt.global_object().get(rt, "FFI".intern())?.get_jsobject();
    let structure = Structure::new_indexed(rt, Some(proto), false);
    let mut obj = JsObject::new(rt, &structure, FFILibrary::get_class(), ObjectTag::Ordinary);
    unsafe {
        (obj.data::<FFILibrary>() as *mut ManuallyDrop<FFILibrary> as *mut FFILibrary).write(lib);
    }
    Ok(JsValue::new(obj))
}

pub fn ffi_function_attach(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    let func = unsafe {
        let lib = {
            let val = args.at(0);
            if !val.is_jsobject() {
                let msg = JsString::new(rt, "function_attach requires library object");
                return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
            }
            let val = val.get_jsobject();
            if !val.is_class(FFILibrary::get_class()) {
                let msg = JsString::new(rt, "function_attach requires library object");
                return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
            }
            val
        };

        let name = { args.at(1).to_string(rt)? };
        root!(rnames = stack, vec![]);
        let args_ = {
            let names = args.at(2);
            if !names.is_jsobject() {
                let msg = JsString::new(
                    rt,
                    "function_attach requires array-like object of arguments",
                );
                return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
            }

            root!(names = stack, names.get_jsobject());
            let len = super::get_length(rt, &mut names)?;

            for i in 0..len {
                rnames.push(names.get(rt, Symbol::Index(i))?);
            }
            rnames
        };

        FFIFunction::attach(rt, lib.data::<FFILibrary>(), &name, &args_, args.at(3))?
    };

    Ok(JsValue::new(func))
}

pub fn ffi_function_call(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let stack = rt.shadowstack();
    rt.gc().defer();
    let func = unsafe {
        let val = args.this;
        if !val.is_jsobject() {
            let msg = JsString::new(rt, "call requires function object");
            return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
        }
        let val = val.get_jsobject();
        if !val.is_class(FFIFunction::get_class()) {
            let msg = JsString::new(rt, "CALL requires FFIFunction object");
            return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
        }
        val
    };
    root!(rnames = stack, vec![]);

    let args = {
        let names = args.at(0);
        if !names.is_jsobject() {
            let msg = JsString::new(rt, "function call requires array-like object of arguments");
            return Err(JsValue::new(JsTypeError::new(rt, msg, None)));
        }

        root!(names = stack, names.get_jsobject());
        let len = super::get_length(rt, &mut names)?;

        for i in 0..len {
            rnames.push(names.get(rt, Symbol::Index(i))?);
        }
        rnames
    };
    root!(res = stack, unsafe {
        func.data::<FFIFunction>().call(rt, &args)
    });

    rt.gc().undefer();
    // can't just do `*res` since it is internally Pin<&mut Result<JsValue,JsValue>>`
    match &*res {
        Ok(val) => Ok(*val),
        Err(e) => Err(*e),
    }
}
