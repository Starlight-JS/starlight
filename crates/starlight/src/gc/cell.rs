/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */
use crate::{
    gc::snapshot::{deserializer::Deserializable, serializer::Serializable},
    prelude::SnapshotSerializer,
};
use mopa::mopafy;
use std::{
    any::TypeId,
    collections::HashMap,
    marker::PhantomData,
    mem::size_of,
    mem::transmute,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
use wtf_rs::tagged_ptr::TaggedPointer;

pub trait Visitor {
    fn visit(&mut self, cell: GcPointer<dyn GcCell>);
    fn visit_raw(&mut self, cell: *mut GcPointerBase);
    /// Add memory range to search for conservative roots. Note that some collectors might scan this range multiple
    /// times if you supplied same range multiple times.
    fn add_conservative(&mut self, from: usize, to: usize);
    fn visit_weak(&mut self, at: *const WeakSlot);
}

/// Indicates that a type can be traced by a garbage collector.
///
/// This doesn't necessarily mean that the type is safe to allocate in a garbage collector ([GcCell]).
///
/// ## Safety
/// See the documentation of the `trace` method for more info.
/// Essentially, this object must faithfully trace anything that
/// could contain garbage collected pointers or other `Trace` items.
pub unsafe trait Trace {
    /// Visit each field in this type
    ///
    ///
    /// Structures should trace each of their fields,
    /// and collections should trace each of their elements.
    ///
    /// ### Safety
    /// Some types (like `GcPointer`) need special actions taken when they're traced,
    /// but those are somewhat rare and are usually already provided by the garbage collector.
    ///
    /// ## Always Permitted
    /// - Reading your own memory (includes iteration)
    ///   - Interior mutation is undefined behavior, even if you use `RefCell`
    /// - Panicking
    ///   - This should be reserved for cases where you are seriously screwed up,
    ///       and can't fulfill your contract to trace your interior properly.
    ///   - This rule may change in future versions, depending on how we deal with multi-threading.
    /// ## Never Permitted Behavior
    /// - Forgetting a element of a collection, or field of a structure
    ///   - If you forget an element undefined behavior will result
    ///   - This is why we always prefer automatically derived implementations where possible.
    ///     - You will never trigger undefined behavior with an automatic implementation,
    ///       and it'll always be completely sufficient for safe code (aside from destructors).
    ///     - With an automatically derived implementation you will never miss a field
    /// - Invoking this function directly.
    fn trace(&self, visitor: &mut Visitor) {
        let _ = visitor;
    }
}

/// `GcCell` is a type that can be allocated in GC gc and passed to JavaScript environment.
///
///
/// All cells that is not part of `src/vm` treatened as dummy objects and property accesses
/// is no-op on them.
///
pub trait GcCell: mopa::Any + Trace + Serializable + Unpin {
    /// Used when object has dynamic size i.e arrays
    fn compute_size(&self) -> usize {
        std::mem::size_of_val(self)
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }

    fn deser_pair(&self) -> (usize, usize);
}

mopafy!(GcCell);
#[no_mangle]
pub unsafe extern "C" fn get_jscell_type_id(x: *mut GcPointerBase) -> u64 {
    transmute((*x).get_dyn().type_id())
}
#[repr(C)]
pub struct GcPointerBase {
    pub vtable: TaggedPointer<8>,
    pub type_id: TypeId,
}

pub const POSSIBLY_BLACK: u8 = 0;
pub const POSSIBLY_GREY: u8 = 2;
pub const DEFINETELY_WHITE: u8 = 1;

impl GcPointerBase {
    pub fn vtable_offsetof() -> usize {
        offsetof!(GcPointerBase.vtable)
    }
    pub fn typeid_offsetof() -> usize {
        offsetof!(GcPointerBase.type_id)
    }
    pub fn allocation_size(&self) -> usize {
        self.get_dyn().compute_size() + size_of::<Self>()
    }
    pub fn new(vtable: usize, type_id: TypeId) -> Self {
        Self {
            vtable: TaggedPointer::new(vtable as *const u8, DEFINETELY_WHITE as _),
            type_id,
        }
    }

    pub fn state(&self) -> u8 {
        self.vtable.tag() as _
        //self.cell_state.load(Ordering::Acquire)
    }

    pub fn set_state(&mut self, from: u8, to: u8) -> bool {
        self.vtable.compare_and_set_tag(from as _, to as _)
    }
    pub fn force_set_state(&mut self, to: u8) {
        self.vtable.set_tag(to as _);
    }
    pub fn data<T>(&self) -> *mut T {
        unsafe {
            (self as *const Self as *mut u8)
                .add(size_of::<Self>())
                .cast()
        }
    }
    pub fn raw(&self) -> usize {
        self.vtable.get_ptr()
    }

    pub fn get_dyn(&self) -> &mut dyn GcCell {
        unsafe {
            std::mem::transmute(mopa::TraitObject {
                vtable: self.vtable() as _,
                data: self.data::<u8>() as _,
            })
        }
    }

    pub fn vtable(&self) -> usize {
        self.vtable.get_ptr()
    }
}
pub fn vtable_of<T: GcCell>(x: *const T) -> usize {
    unsafe { core::mem::transmute::<_, mopa::TraitObject>(x as *const dyn GcCell).vtable as _ }
}

pub fn vtable_of_type<T: GcCell + Sized>() -> usize {
    vtable_of(core::ptr::null::<T>())
}

/// A garbage collected pointer to a value.
///
/// This is the equivalent of a garbage collected smart-pointer.
///
///
/// The smart pointer is simply a guarantee to the garbage collector
/// that this points to a garbage collected object with the correct header,
/// and not some arbitrary bits that you've decided to gc allocate.]
#[repr(transparent)]
pub struct GcPointer<T: ?Sized> {
    pub(crate) base: NonNull<GcPointerBase>,
    pub(crate) marker: PhantomData<T>,
}

impl<T: GcCell + ?Sized> GcPointer<T> {
    pub fn ptr_eq<U: GcCell + ?Sized>(this: &Self, other: &GcPointer<U>) -> bool {
        this.base == other.base
    }
    #[inline]
    pub fn as_dyn(self) -> GcPointer<dyn GcCell> {
        GcPointer {
            base: self.base,
            marker: PhantomData,
        }
    }
}

impl<T: GcCell + ?Sized> GcPointer<T> {
    #[inline]
    pub fn is<U: GcCell>(self) -> bool {
        unsafe { (*self.base.as_ptr()).type_id == TypeId::of::<U>() }
    }

    #[inline]
    pub fn get_dyn(&self) -> &dyn GcCell {
        unsafe { (*self.base.as_ptr()).get_dyn() }
    }

    #[inline]
    pub fn get_dyn_mut(&mut self) -> &mut dyn GcCell {
        unsafe { (*self.base.as_ptr()).get_dyn() }
    }

    #[inline]
    pub unsafe fn downcast_unchecked<U: GcCell>(self) -> GcPointer<U> {
        GcPointer {
            base: self.base,
            marker: PhantomData,
        }
    }

    #[inline]
    pub fn downcast<U: GcCell>(self) -> Option<GcPointer<U>> {
        if !self.is::<U>() {
            None
        } else {
            Some(unsafe { self.downcast_unchecked() })
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WeakState {
    Free = 0,
    Unmarked,
    Mark,
}
pub struct WeakSlot {
    pub(crate) value: Option<GcPointer<dyn GcCell>>,
}

impl Trace for WeakSlot {}
impl GcCell for WeakSlot {}
impl Serializable for WeakSlot {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.value.serialize(serializer);
    }
}

impl Deserializable for WeakSlot {
    unsafe fn deserialize(at: *mut u8, deser: &mut crate::prelude::Deserializer) {
        at.cast::<Self>().write(Self {
            value: deser.read_opt_gc(),
        });
    }
    unsafe fn deserialize_inplace(_deser: &mut crate::prelude::Deserializer) -> Self {
        unreachable!()
    }
    unsafe fn allocate(
        vm: &mut crate::vm::VirtualMachine,
        deser: &mut crate::prelude::Deserializer,
    ) -> *mut GcPointerBase {
        vm.gc.allocate_raw(
            vtable_of_type::<Self>() as _,
            size_of::<Self>(),
            TypeId::of::<Self>(),
        )
    }
}
#[repr(transparent)]
pub struct WeakRef<T: GcCell> {
    pub(crate) slot: GcPointer<WeakSlot>,
    pub(crate) marker: PhantomData<T>,
}

impl<T: GcCell> WeakRef<T> {
    pub fn upgrade(&self) -> Option<GcPointer<T>> {
        self.slot.value.map(|x| unsafe { x.downcast_unchecked() })
    }
}

macro_rules! impl_prim {
    ($($t: ty)*) => {
        $(
            impl Trace for $t {}
            impl GcCell for $t {
                fn deser_pair(&self) -> (usize,usize) {
                    (Self::deserialize as usize,Self::allocate as usize)
                }

            }
        )*
    };
}

impl_prim!(String bool f32 f64 u8 i8 u16 i16 u32 i32 u64 i64 );
unsafe impl<T: Trace> Trace for Vec<T> {
    fn trace(&self, visitor: &mut Visitor) {
        for val in self.iter_mut() {
            val.trace(visitor);
        }
    }
}

unsafe impl<T: GcCell + ?Sized> Trace for GcPointer<T> {
    fn trace(&self, visitor: &mut Visitor) {
        visitor.visit(self.as_dyn());
    }
}

impl<T: GcCell + ?Sized> Copy for GcPointer<T> {}
impl<T: GcCell + ?Sized> Clone for GcPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: GcCell> Deref for GcPointer<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(&*self.base.as_ptr()).data::<T>() }
    }
}
impl<T: GcCell> DerefMut for GcPointer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(&*self.base.as_ptr()).data::<T>() }
    }
}

impl<T: GcCell> std::fmt::Pointer for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:p}", self.base)
    }
}

impl<T: GcCell + std::fmt::Debug> std::fmt::Debug for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", **self)
    }
}
impl<T: GcCell + std::fmt::Display> std::fmt::Display for GcPointer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", **self)
    }
}

impl<T: GcCell> GcCell for WeakRef<T> {}
unsafe impl<T: GcCell> Trace for WeakRef<T> {
    fn trace(&self, visitor: &mut Visitor) {
        self.slot.trace(visitor);
    }
}

#[allow(mutable_transmutes)]
unsafe impl<K: Trace, V: Trace> Trace for HashMap<K, V> {
    fn trace(&self, visitor: &mut Visitor) {
        for (key, value) in self.iter_mut() {
            unsafe {
                // TODO: This is really  unsafe. We transmute reference to mutable reference for tracing which is
                // very unsafe, we should find better alternative to this.
                let km = std::mem::transmute::<_, &mut K>(key);
                km.trace(visitor);
            }
            //key.trace(visitor);
            value.trace(visitor);
        }
    }
}

impl<
        K: GcCell + Eq + std::hash::Hash + Trace + 'static + Serializable + Deserializable,
        V: GcCell + Trace + 'static + Serializable + Deserializable,
    > GcCell for HashMap<K, V>
{
}

unsafe impl<T: Trace> Trace for Option<T> {
    fn trace(&self, visitor: &mut Visitor) {
        match self {
            Some(val) => val.trace(visitor),
            _ => (),
        }
    }
}

impl<T: GcCell + Serializable + 'static + Deserializable> GcCell for Vec<T> {
    fn deser_pair(&self) -> (usize, usize) {
        (Self::deserialize as usize, Self::allocate as usize)
    }
}
impl<T: GcCell + ?Sized> GcCell for GcPointer<T> {}

impl<T: GcCell + Serializable + Deserializable + 'static> GcCell for Option<T> {}

impl<T: GcCell> Copy for WeakRef<T> {}
impl<T: GcCell> Clone for WeakRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

unsafe impl<T: Trace, E: Trace> Trace for Result<T, E> {
    fn trace(&self, visitor: &mut Visitor) {
        match self {
            Ok(x) => x.trace(visitor),
            Err(e) => e.trace(visitor),
        }
    }
}

impl<A: GcCell + Deserializable, B: GcCell + Deserializable> GcCell for (A, B) {
    fn compute_size(&self) -> usize {
        self.0.compute_size() + self.1.compute_size()
    }
}

impl<A: GcCell, B: GcCell> Serializable for (A, B) {
    fn serialize(&self, serializer: &mut SnapshotSerializer) {
        self.0.serialize(serializer);
        self.1.serialize(serializer);
    }
}
unsafe impl<A: Trace, B: Trace> Trace for (A, B) {
    fn trace(&self, visitor: &mut Visitor) {
        self.0.trace(visitor);
        self.1.trace(visitor);
    }
}

impl<T: GcCell> PartialEq for GcPointer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl<T: GcCell> Eq for GcPointer<T> {}

#[repr(C)]
pub struct ObjectVTable {
    pub real_vtable: *const (),
    pub type_id: TypeId,
}

pub trait GetVTable {
    fn vtable() -> &'static ObjectVTable;
}

impl<T: GcCell> GetVTable for T {
    fn vtable() -> &'static ObjectVTable {
        static mut VTABLE: ObjectVTable = ObjectVTable {
            real_vtable: 0 as _,
            type_id: TypeId::of::<()>(),
        };
        unsafe {
            VTABLE.real_vtable = vtable_of_type::<Self>() as _;
            VTABLE.type_id = TypeId::of::<T>()
        };
        unsafe { &VTABLE }
    }
}
pub fn vtable_of_i32() -> &'static ObjectVTable {
    i32::vtable()
}
