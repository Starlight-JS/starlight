/*
pub trait TypedArrayType: Default + Copy + Deserializable + Serializable + GcCell + Unpin {
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue>;
    fn into_jsvalue(self, vm: &mut Runtime) -> Result<JsValue, JsValue>;

    #[inline]
    unsafe fn fill(start: *mut Self, end: *mut Self, fill: Self) {
        let mut cur = start;
        while cur != end {
            cur.write(fill);
            cur = cur.add(1);
        }
    }

    #[inline]
    unsafe fn uninit_copy(
        mut first: *mut Self,
        last: *mut Self,
        mut result: *mut Self,
    ) -> *mut Self {
        while first != last {
            result.write(first.read());
            first = first.add(1);
            result = result.add(1);
        }
        result
    }

    #[inline]
    unsafe fn copy_backward(
        first: *mut Self,
        mut last: *mut Self,
        mut result: *mut Self,
    ) -> *mut Self {
        while first != last {
            last = last.sub(1);
            result = result.sub(1);
            result.write(last.read());
        }
        result
    }
    #[inline]
    unsafe fn copy(mut first: *mut Self, last: *mut Self, mut result: *mut Self) -> *mut Self {
        while first != last {
            result.write(first.read());
            first = first.add(1);
            result = result.add(1);
        }
        result
    }
}
/// A GC-managed resizable vector of values. It is used for storage of property
/// values in objects and also indexed property values in arrays. It supports
/// resizing on both ends which is necessary for the simplest implementation of
/// JavaScript arrays (using a base offset and length).
#[repr(C)]
pub struct TypedArrayStorage<T: TypedArrayType> {
    pub(crate) size: u32,
    pub(crate) capacity: u32,
    pub(crate) data: [T; 0],
}

impl<T: TypedArrayType> GcPointer<TypedArrayStorage<T>> {
    pub fn resize_within_capacity(&mut self, _rt: &mut Heap, new_size: u32) {
        assert!(
            new_size <= self.capacity(),
            "new_size must be <= capacity in resize_Within_capacity"
        );

        let sz = self.size();
        unsafe {
            if new_size > sz {
                T::fill(
                    self.data_mut().add(sz as _),
                    self.data_mut().add(new_size as _),
                    T::default(),
                );
            }
        }
        self.size = new_size;
    }

    pub fn ensure_capacity(&mut self, rt: &mut Heap, capacity: u32) {
        assert!(
            capacity <= u32::MAX as u32,
            "capacity overflows 32-bit storage"
        );

        if capacity <= self.capacity() {
            return;
        }

        unsafe { self.reallocate_to_larger(rt, capacity, 0, 0, self.size()) }
    }
    pub fn resize(&mut self, rt: &mut Heap, new_size: u32) {
        self.shift(rt, 0, 0, new_size)
    }

    #[cold]
    pub fn push_back_slowpath(&mut self, rt: &mut Heap, value: T) {
        let size = self.size();

        self.resize(rt, self.size() + 1);
        *self.at_mut(size) = value;
    }

    pub fn push_back(&mut self, rt: &mut Heap, value: T) {
        let currsz = self.size();
        if currsz < self.capacity() {
            unsafe {
                self.data_mut().add(currsz as _).write(value);
                self.size = currsz + 1;
            }
            return;
        }
        self.push_back_slowpath(rt, value)
    }

    pub fn pop_back(&mut self, _rt: &mut Heap) -> T {
        let sz = self.size();
        assert!(sz > 0, "empty ArrayStorage");

        unsafe {
            let val = self.data().add(sz as usize - 1).read();
            self.size = sz - 1;
            val
        }
    }

    pub fn shift(&mut self, rt: &mut Heap, from_first: u32, to_first: u32, to_last: u32) {
        assert!(to_first <= to_last, "First must be before last");
        assert!(from_first <= self.size, "from_first must be before size");
        unsafe {
            if to_last <= self.capacity() {
                let copy_size = std::cmp::min(self.size() - from_first, to_last - to_first);
                if from_first > to_first {
                    T::copy(
                        self.data_mut().add(from_first as usize),
                        self.data_mut()
                            .add(from_first as usize + copy_size as usize),
                        self.data_mut().add(to_first as usize),
                    );
                } else if from_first < to_first {
                    T::copy_backward(
                        self.data_mut().add(from_first as usize),
                        self.data_mut()
                            .add(from_first as usize + copy_size as usize),
                        self.data_mut().add(to_first as _),
                    );
                }
                T::fill(
                    self.data_mut().add(to_first as usize + copy_size as usize),
                    self.data_mut().add(to_last as usize),
                    T::default(),
                );
                self.size = to_last;
                return;
            }

            let mut capacity = self.capacity();
            if capacity < TypedArrayStorage::<T>::max_elements() as u32 / 2 {
                capacity = std::cmp::max(capacity * 2, to_last);
            } else {
                capacity = TypedArrayStorage::<T>::max_elements() as u32;
            }
            self.reallocate_to_larger(rt, capacity, from_first, to_first, to_last)
        }
    }

    pub unsafe fn reallocate_to_larger(
        &mut self,
        rt: &mut Heap,
        capacity: u32,
        from_first: u32,
        to_first: u32,
        to_last: u32,
    ) {
        assert!(capacity > self.capacity());

        let mut arr_res = TypedArrayStorage::<T>::new(rt, capacity);
        let copy_size = std::cmp::min(self.size() - from_first, to_last - to_first);

        {
            let from = self.data_mut().add(from_first as _);
            let to = arr_res.data_mut().add(to_first as _);
            T::uninit_copy(from, from.add(copy_size as _), to);
        }

        T::fill(
            arr_res.data_mut(),
            arr_res.data_mut().add(to_first as _),
            T::default(),
        );

        if to_first + copy_size < to_last {
            T::fill(
                arr_res
                    .data_mut()
                    .add(to_first as usize + copy_size as usize),
                arr_res.data_mut().add(to_last as usize),
                T::default(),
            );
        }

        arr_res.size = to_last;
        *self = arr_res;
    }
}

impl<T: TypedArrayType> TypedArrayStorage<T> {
    pub fn max_elements() -> usize {
        (u32::MAX as usize - 8) / size_of::<T>()
    }
    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    pub fn with_size(rt: &mut Runtime, size: u32, capacity: u32) -> GcPointer<Self> {
        let stack = rt.shadowstack();
        crate::letroot!(this = stack, Self::new(rt.heap(), capacity));
        this.resize_within_capacity(rt.heap(), size);
        *this
    }
    pub fn new(rt: &mut Heap, capacity: u32) -> GcPointer<Self> {
        let cell = rt.allocate(Self {
            capacity,
            size: 0,
            data: [],
        });

        cell
    }
    pub fn data(&self) -> *const T {
        self.data.as_ptr()
    }
    pub fn as_slice(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.data(), self.size as _) }
    }

    pub fn data_mut(&mut self) -> *mut T {
        self.data.as_mut_ptr()
    }
    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.data_mut(), self.size as _) }
    }
    pub fn at(&self, index: u32) -> &T {
        assert!(index < self.size(), "index out of range");
        unsafe { &*self.data().add(index as _) }
    }
    pub fn at_mut(&mut self, index: u32) -> &mut T {
        assert!(index < self.size(), "index out of range");
        unsafe { &mut *self.data_mut().add(index as _) }
    }
}

unsafe impl<T: TypedArrayType> Trace for TypedArrayStorage<T> {
    fn trace(&mut self, visitor: &mut dyn Tracer) {
        self.as_slice_mut().iter_mut().for_each(|value| {
            value.trace(visitor);
        });
    }
}

impl<T: TypedArrayType> GcCell for TypedArrayStorage<T> {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as _, Self::allocate as _)
    }
    fn compute_size(&self) -> usize {
        (self.capacity as usize * size_of::<T>()) + size_of::<Self>()
    }
}

impl<T: TypedArrayType> Serializable for TypedArrayStorage<T> {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.capacity.serialize(serializer);
        self.size.serialize(serializer);
        for item in self.as_slice().iter() {
            item.serialize(serializer);
        }
    }
}

impl<T: TypedArrayType> Deserializable for TypedArrayStorage<T> {
    unsafe fn allocate(rt: &mut Runtime, deser: &mut Deserializer) -> *mut GcPointerBase {
        let cap = u32::deserialize_inplace(deser);
        deser.pc -= 4;
        rt.heap().allocate_raw(
            vtable_of_type::<Self>() as _,
            cap as usize * size_of::<T>() + size_of::<Self>() + 16,
            TypeId::of::<Self>(),
        )
    }
    unsafe fn deserialize(at: *mut u8, deser: &mut Deserializer) {
        let cap = u32::deserialize_inplace(deser);
        let size = u32::deserialize_inplace(deser);
        let mut arr = GcPointer::<TypedArrayStorage<T>> {
            base: NonNull::new_unchecked(at.sub(size_of::<GcPointerBase>()).cast()),
            marker: PhantomData,
        };
        arr.capacity = cap;

        for _ in 0..size {
            let item = T::deserialize_inplace(deser);
            arr.push_back((&mut *deser.rt).heap(), item);
        }
        assert_eq!(
            arr.size, size,
            "cap {}, size {}, found {},{}",
            cap, size, arr.size, arr.capacity
        );
        assert_eq!(arr.capacity, cap);
    }
    unsafe fn deserialize_inplace(_deser: &mut Deserializer) -> Self {
        unreachable!()
    }
    unsafe fn dummy_read(_deser: &mut Deserializer) {
        unreachable!()
    }
}

impl TypedArrayType for u32 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_uint32(vm)
    }
}

impl TypedArrayType for u16 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_uint32(vm).map(|x| x as Self)
    }
}
impl TypedArrayType for u8 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_uint32(vm).map(|x| x as Self)
    }
}

impl TypedArrayType for i8 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_int32(vm).map(|x| x as Self)
    }
}

impl TypedArrayType for i16 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_int32(vm).map(|x| x as Self)
    }
}

impl TypedArrayType for i32 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::encode_int32(self as i32))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        val.to_int32(vm).map(|x| x as Self)
    }
}
impl TypedArrayType for i64 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::new(self))
    }

    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        Ok(val.to_number(vm)? as _)
    }
}

impl TypedArrayType for u64 {
    fn into_jsvalue(self, _vm: &mut Runtime) -> Result<JsValue, JsValue> {
        Ok(JsValue::new(self as f64))
    }
    fn from_jsvalue(vm: &mut Runtime, val: JsValue) -> Result<Self, JsValue> {
        Ok(val.to_number(vm)? as u64)
    }
}

#[cfg(test)]
mod tests {
    use crate::gc::migc::MiGC;

    use super::*;
    #[test]
    fn test_ser_deser() {
        let mut rt = Platform::new_runtime(RuntimeParams::default(), GcParams::default(), None);

        let mut my_typed_array = TypedArrayStorage::<u32>::new(rt.heap(), 100);
        my_typed_array.push_back(rt.heap(), 42);

        assert_eq!(*my_typed_array.at(0), 42);
        rt.global_object()
            .put(
                &mut rt,
                "myTypedArray".intern(),
                JsValue::encode_object_value(my_typed_array),
                false,
            )
            .unwrap_or_else(|_| unreachable!());

        let snapshot = Snapshot::take(false, &mut rt, |_, _| {});

        let mut rt = Deserializer::deserialize(
            false,
            &snapshot.buffer,
            RuntimeParams::default(),
            Heap::new(MiGC::new(GcParams::default())),
            None,
            |_, _| {},
        );

        let my_typed_array = rt.get_global("myTypedArray").unwrap();
        let object = my_typed_array
            .get_object()
            .downcast::<TypedArrayStorage<u32>>()
            .unwrap();
        assert_eq!(*object.at(0), 42);
    }
}
*/
#[allow(dead_code)]
pub struct JsTypedArrayBase {
    length: usize,
    byte_width: u8,
    offset: usize,
}
