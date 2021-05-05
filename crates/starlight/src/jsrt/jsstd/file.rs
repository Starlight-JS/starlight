use crate::define_jsclass_with_symbol;
use crate::prelude::*;
use crate::{
    gc::cell::GcPointer,
    vm::{object::JsObject, Runtime},
};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    mem::ManuallyDrop,
};

pub(super) fn std_init_file(rt: &mut Runtime, mut std: GcPointer<JsObject>) -> Result<(), JsValue> {
    let mut ctor = JsNativeFunction::new(rt, "File".intern(), std_file_open, 2);

    let mut proto = JsObject::new_empty(rt);
    def_native_method!(rt, proto, read, std_file_read, 0)?;
    rt.global_object()
        .put(rt, "@@File".intern().private(), JsValue::new(proto), false)?;
    ctor.put(rt, "prototype".intern(), JsValue::new(proto), false)?;
    std.put(rt, "File".intern(), JsValue::new(ctor), false)?;
    Ok(())
}

pub fn std_file_open(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let path = args.at(0).to_string(rt)?;
    let flags = if args.at(1).is_string() {
        args.at(1).to_string(rt)?
    } else {
        "".to_string()
    };
    let path = std::path::Path::new(&path).canonicalize().unwrap();
    
    let file = match OpenOptions::new()
        .write(flags.contains("w"))
        .read(flags.contains("r"))
        .append(flags.contains("a"))
        .create(flags.contains("+"))
        .open(&path).unwrap()
    {
        file=> file,
        /*Err(e) => {
            return Err(JsValue::new(rt.new_reference_error(format!(
                "Failed to open file '{}': {}",
                path.display(), e
            ))))
        }*/
    };

    let proto = rt
        .global_object()
        .get(rt, "@@File".intern().private())?
        .to_object(rt)?;
    let structure = Structure::new_indexed(rt, Some(proto), false);
    let mut obj = JsObject::new(rt, &structure, FileObject::get_class(), ObjectTag::Ordinary);
    *obj.data::<FileObject>() = ManuallyDrop::new(FileObject { file });
    Ok(JsValue::new(obj))
}

/// std.File.prototype.read simply reads file contents to string.
pub fn std_file_read(rt: &mut Runtime, args: &Arguments) -> Result<JsValue, JsValue> {
    let this = args.this.to_object(rt)?;
    if this.is_class(FileObject::get_class()) {
        let mut buffer = String::new();
        match this.data::<FileObject>().file.read_to_string(&mut buffer) {
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
pub struct FileObject {
    pub file: File,
}

extern "C" fn drop_file_fn(obj: &mut JsObject) {
    unsafe { ManuallyDrop::drop(obj.data::<File>()) }
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
