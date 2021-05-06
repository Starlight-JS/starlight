use crate::define_jsclass_with_symbol;
use crate::prelude::*;
use crate::{
    gc::cell::GcPointer,
    vm::{object::JsObject, Runtime},
};
use std::{
    fs::{File, OpenOptions},
    intrinsics::unlikely,
    io::{Read, Write},
    mem::ManuallyDrop,
};

pub(super) fn std_init_file(rt: &mut Runtime, mut std: GcPointer<JsObject>) -> Result<(), JsValue> {
    let mut ctor = JsNativeFunction::new(rt, "File".intern(), std_file_open, 2);

    let mut proto = JsObject::new_empty(rt);
    def_native_method!(rt, proto, read, std_file_read, 0)?;
    def_native_method!(rt, proto, write, std_file_write, 1)?;
    def_native_method!(rt, proto, writeAll, std_file_write_all, 1)?;
    def_native_method!(rt, proto, readBytes, std_file_read_bytes, 0)?;
    def_native_method!(rt, proto, readBytesExact, std_file_read_bytes_exact, 1)?;
    def_native_method!(rt, proto, readBytesToEnd, std_file_read_bytes_to_end, 0)?;
    def_native_method!(rt, proto, close, std_file_close, 0)?;
    rt.global_object()
        .put(rt, "@@File".intern().private(), JsValue::new(proto), false)?;
    ctor.put(rt, "prototype".intern(), JsValue::new(proto), false)?;
    std.put(rt, "File".intern(), JsValue::new(ctor), false)?;
    Ok(())
}

pub fn std_file_open(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let path = args.at(0).to_string(rt)?;
    let flags = if args.at(1).is_jsstring() {
        args.at(1).to_string(rt)?
    } else {
        "".to_string()
    };

    let path = std::path::Path::new(&path);
    let mut opts = OpenOptions::new();

    let mut opts_ = opts
        .read(flags.contains('r'))
        .write(flags.contains('w'))
        .create(flags.contains('+'))
        .append(flags.contains('a'))
        .truncate(flags.contains('t'));

    let file = match opts_.open(&path) {
        Ok(file) => file,
        Err(e) => {
            return Err(JsValue::new(rt.new_reference_error(format!(
                "Failed to open file '{}': {}",
                path.display(),
                e
            ))))
        }
    };

    let proto = rt
        .global_object()
        .get(rt, "@@File".intern().private())?
        .to_object(rt)?;
    let structure = Structure::new_indexed(rt, Some(proto), false);
    let mut obj = JsObject::new(rt, &structure, FileObject::get_class(), ObjectTag::Ordinary);
    *obj.data::<FileObject>() = ManuallyDrop::new(FileObject { file: Some(file) });
    Ok(JsValue::new(obj))
}

/// std.File.prototype.write takes array-like object or string to write to file
/// and returns count of bytes written.
pub fn std_file_write(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if unlikely(!this.is_class(FileObject::get_class())) {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.write requires file object"),
        ));
    }
    let mut buffer: Vec<u8>;
    if args.at(0).is_jsobject() {
        let stack = rt.shadowstack();
        letroot!(buffer_object = stack, args.at(0).get_jsobject());
        let length = crate::jsrt::get_length(rt, &mut buffer_object)?;
        buffer = Vec::with_capacity(length as _);
        for i in 0..length {
            let uint = buffer_object.get(rt, Symbol::Index(i))?.to_uint32(rt)?;
            if uint <= u8::MAX as u32 {
                buffer.push(uint as u8);
            } else if uint <= u16::MAX as u32 {
                let ne = (uint as u16).to_ne_bytes();
                buffer.push(ne[0]);
                buffer.push(ne[1]);
            } else {
                let ne = (uint as u32).to_ne_bytes();
                buffer.extend(&ne);
            }
        }
    } else {
        let string = args.at(0).to_string(rt)?;
        buffer = string.as_bytes().to_vec();
    }
    let file = match this.data::<FileObject>().file {
        Some(ref mut file) => file,
        None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
    };
    match file.write(&mut buffer) {
        Ok(count) => Ok(JsValue::new(count as u32)),
        Err(e) => Err(JsValue::new(JsString::new(rt, e.to_string()))),
    }
}

/// std.File.prototype.writeAll takes array-like object or string to write to file.
pub fn std_file_write_all(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if unlikely(!this.is_class(FileObject::get_class())) {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.write requires file object"),
        ));
    }
    let mut buffer: Vec<u8>;
    if args.at(0).is_jsobject() {
        let stack = rt.shadowstack();
        letroot!(buffer_object = stack, args.at(0).get_jsobject());
        let length = crate::jsrt::get_length(rt, &mut buffer_object)?;
        buffer = Vec::with_capacity(length as _);
        for i in 0..length {
            let uint = buffer_object.get(rt, Symbol::Index(i))?.to_uint32(rt)?;
            if uint <= u8::MAX as u32 {
                buffer.push(uint as u8);
            } else if uint <= u16::MAX as u32 {
                let ne = (uint as u16).to_ne_bytes();
                buffer.push(ne[0]);
                buffer.push(ne[1]);
            } else {
                let ne = (uint as u32).to_ne_bytes();
                buffer.extend(&ne);
            }
        }
    } else {
        let string = args.at(0).to_string(rt)?;
        buffer = string.as_bytes().to_vec();
    }

    let file = match this.data::<FileObject>().file {
        Some(ref mut file) => file,
        None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
    };
    match file.write_all(&mut buffer) {
        Ok(_) => Ok(JsValue::new(())),
        Err(e) => Err(JsValue::new(JsString::new(rt, e.to_string()))),
    }
}

/// std.File.prototype.read simply reads file contents to string.
pub fn std_file_read(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if this.is_class(FileObject::get_class()) {
        let mut buffer = String::new();
        let file = match this.data::<FileObject>().file {
            Some(ref mut file) => file,
            None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
        };
        match file.read_to_string(&mut buffer) {
            Ok(_) => (),
            Err(e) => {
                return Err(JsValue::new(JsString::new(
                    rt,
                    format!("failed to read file contents to string: {}", e),
                )))
            }
        }
        Ok(JsValue::new(JsString::new(rt, buffer)))
    } else {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.read requires file object"),
        ));
    }
}

/// std.File.prototype.readBytes: returns array of bytes that was read from file
pub fn std_file_read_bytes(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if this.is_class(FileObject::get_class()) {
        let mut buffer = Vec::new();
        let file = match this.data::<FileObject>().file {
            Some(ref mut file) => file,
            None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
        };
        match file.read(&mut buffer) {
            Ok(_) => {
                let mut arr = JsArray::new(rt, buffer.len() as _);
                for (index, byte) in buffer.iter().enumerate() {
                    arr.put(rt, Symbol::Index(index as _), JsValue::new(*byte), false)?;
                }

                return Ok(JsValue::new(arr));
            }
            Err(e) => {
                return Err(JsValue::new(JsString::new(
                    rt,
                    format!("failed to read file contents to string: {}", e),
                )))
            }
        }
    } else {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.read requires file object"),
        ));
    }
}

/// std.File.prototype.readBytesToEnd: returns array of bytes that was read from file
pub fn std_file_read_bytes_to_end(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if this.is_class(FileObject::get_class()) {
        let mut buffer = Vec::new();
        let file = match this.data::<FileObject>().file {
            Some(ref mut file) => file,
            None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
        };
        match file.read_to_end(&mut buffer) {
            Ok(_) => {
                let mut arr = JsArray::new(rt, buffer.len() as _);
                for (index, byte) in buffer.iter().enumerate() {
                    arr.put(rt, Symbol::Index(index as _), JsValue::new(*byte), false)?;
                }

                return Ok(JsValue::new(arr));
            }
            Err(e) => {
                return Err(JsValue::new(JsString::new(
                    rt,
                    format!("failed to read file contents to string: {}", e),
                )))
            }
        }
    } else {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.read requires file object"),
        ));
    }
}

/// std.File.prototype.readBytesExact: returns array of bytes that was read from file
pub fn std_file_read_bytes_exact(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    let count = args.at(0).to_uint32(rt)?;
    if this.is_class(FileObject::get_class()) {
        let mut buffer = vec![0u8; count as usize];
        let file = match this.data::<FileObject>().file {
            Some(ref mut file) => file,
            None => return Err(JsValue::new(JsString::new(rt, "File closed"))),
        };
        match file.read_exact(&mut buffer) {
            Ok(_) => {
                let mut arr = JsArray::new(rt, buffer.len() as _);
                for (index, byte) in buffer.iter().enumerate() {
                    arr.put(rt, Symbol::Index(index as _), JsValue::new(*byte), false)?;
                }

                return Ok(JsValue::new(arr));
            }
            Err(e) => {
                return Err(JsValue::new(JsString::new(
                    rt,
                    format!("failed to read file contents to string: {}", e),
                )))
            }
        }
    } else {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.read requires file object"),
        ));
    }
}

pub fn std_file_close(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if this.is_class(FileObject::get_class()) {
        let file = match this.data::<FileObject>().file.take() {
            Some(file) => file,
            None => return Err(JsValue::new(JsString::new(rt, "File already closed"))),
        };
        drop(file);

        Ok(JsValue::new(0))
    } else {
        return Err(JsValue::new(
            rt.new_type_error("std.File.prototype.read requires file object"),
        ));
    }
}

pub struct FileObject {
    pub file: Option<File>,
}

extern "C" fn drop_file_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<FileObject>()) }
}

extern "C" fn deser(_: &mut JsObject, _: &mut Deserializer, _: &mut Runtime) {
    unreachable!("Cannot deserialize file");
}

extern "C" fn ser(_: &JsObject, _: &mut SnapshotSerializer) {
    unreachable!("Cannot serialize file");
}
extern "C" fn fsz() -> usize {
    std::mem::size_of::<FileObject>()
}
impl FileObject {
    define_jsclass_with_symbol!(
        JsObject,
        FFIFunction,
        Object,
        Some(drop_file_fn),
        None,
        Some(deser),
        Some(ser),
        Some(fsz)
    );
}
