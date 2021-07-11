/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::vm::{Runtime, RuntimeRef};

use self::serializer::SnapshotSerializer;

pub mod deserializer;
pub mod serializer;

pub struct Snapshot {
    pub buffer: Box<[u8]>,
}

impl Snapshot {
    pub fn take(
        log: bool,
        runtime: &mut Runtime,
        callback: impl FnOnce(&mut SnapshotSerializer, &mut Runtime),
    ) -> Self {
        let mut serializer =
            serializer::SnapshotSerializer::new(RuntimeRef(runtime as *mut _), log);
        let ids_patch = serializer.output.len();
        serializer.write_u32(0);
        serializer.build_reference_map(runtime);
        serializer.build_symbol_table();
        serializer.build_heap_reference_map(runtime);
        serializer.serialize(runtime);
        callback(&mut serializer, runtime);
        let buf = (serializer.reference_map.len() as u32).to_le_bytes();
        serializer.output[ids_patch] = buf[0];
        serializer.output[ids_patch + 1] = buf[1];
        serializer.output[ids_patch + 2] = buf[2];
        serializer.output[ids_patch + 3] = buf[3];

        Snapshot {
            buffer: serializer.output.into_boxed_slice(),
        }
    }
}
