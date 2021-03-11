use std::{
    collections::HashMap,
    io::{Cursor, Read},
};

use crate::{heap::cell::GcPointer, vm::symbol_table::Symbol};

pub struct Deserializer {
    reader: Cursor<Vec<u8>>,
    reference_map: HashMap<u32, usize>,
    symbol_map: HashMap<u32, Symbol>,
}

impl Deserializer {
    pub fn get_u32(&mut self) -> u32 {
        let mut buf = [0; 4];
        self.reader.read_exact(&mut buf).unwrap();
        u32::from_le_bytes(buf)
    }

    pub fn get_u8(&mut self) -> u8 {
        let mut buf = [0];
        self.reader.read_exact(&mut buf).unwrap();
        buf[0]
    }

    pub fn get_u16(&mut self) -> u16 {
        let mut buf = [0; 2];
        self.reader.read_exact(&mut buf).unwrap();
        u16::from_le_bytes(buf)
    }

    pub fn get_u64(&mut self) -> u64 {
        let mut buf = [0; 8];
        self.reader.read_exact(&mut buf).unwrap();
        u64::from_le_bytes(buf)
    }
}

pub trait Deserializable {
    fn deserialize(deser: &mut Deserializer) -> GcPointer<Self>;
}
