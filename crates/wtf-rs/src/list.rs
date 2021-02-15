use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::{Extend, FromIterator, IntoIterator};
use std::marker::PhantomData;
use std::mem;
use std::ptr;

pub use cursor::{CursorMut, CursorRef};
pub use iter::{IntoIter, Iter, IterMut};

#[cfg(test)]
extern crate rand;

/// A doubly-linked list with nodes allocated in large owned chunks.
///
/// The difference between this linked list and the one in the standard library is the
/// allocation method. The standard library linked list allocates each node in it's own
/// `Box`, while this allocates a `Vec` with many nodes at a time, and keeps an internal
/// list of unused nodes as well as a list of allocations.
///
/// This has the advantage that the nodes are more likely to be closer to each other on
/// the heap, thus increasing CPU cache efficieny, as well as decreasing the number of
/// allocations. It has the downside that you can't deallocate individual nodes, so the
/// only way to deallocate memory owned by this list is to drop it.
pub struct LinkedList<T> {
    head: *mut LinkedNode<T>,
    tail: *mut LinkedNode<T>,
    len: usize,
    capacity: usize,
    chunk_size: usize,
    allocations: Vec<(*mut LinkedNode<T>, usize)>,
    unused_nodes: *mut LinkedNode<T>,
}

// LinkedLists own their data, so the borrow checker should prevent data races.
unsafe impl<T: Send> Send for LinkedList<T> {}
unsafe impl<T: Sync> Sync for LinkedList<T> {}

pub struct LinkedNode<T> {
    next: *mut LinkedNode<T>,
    prev: *mut LinkedNode<T>,
    value: T,
}

impl<T> LinkedList<T> {
    /// Creates an empty `LinkedList` with a chunk size of 64.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(64, list.chunk_size());
    /// ```
    #[inline]
    pub fn new() -> LinkedList<T> {
        LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            capacity: 0,
            chunk_size: 64,
            allocations: Vec::new(),
            unused_nodes: ptr::null_mut(),
        }
    }
    /// Creates an empty `LinkedList` with a chunk size of 64 and makes a single
    /// allocation with the specified amount of nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let list: LinkedList<u32> = LinkedList::with_capacity(293);
    /// assert_eq!(293, list.capacity());
    /// ```
    #[inline]
    pub fn with_capacity(cap: usize) -> LinkedList<T> {
        let mut list = LinkedList {
            head: ptr::null_mut(),
            tail: ptr::null_mut(),
            len: 0,
            capacity: 0,
            chunk_size: 64,
            allocations: Vec::with_capacity(1),
            unused_nodes: ptr::null_mut(),
        };
        list.allocate(cap);
        list
    }

    /// Add the element to the back of the linked list in `O(1)`, unless it has to
    /// allocate, which is `O(chunk_size)`.
    ///
    /// This will not make any allocation unless `len = capacity`, in which case it will
    /// allocate `chunk_size` nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(0, list.capacity());
    /// // add an element, this will cause an allocation
    /// list.push_back(35);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&35), list.back());
    ///
    /// // if we add another, then since the allocation is large enough, this shouldn't
    /// // change the capacity
    /// list.push_back(29);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&29), list.back());
    /// // the first element should still be at the front of the list
    /// assert_eq!(Some(&35), list.front());
    /// ```
    pub fn push_back(&mut self, value: T) {
        let tail = self.tail;
        let node = self.new_node(ptr::null_mut(), tail, value);

        if self.head.is_null() {
            self.head = node;
        }
        if !self.tail.is_null() {
            unsafe {
                (*self.tail).next = node;
            }
        }

        self.tail = node;
        self.len += 1;
    }
    /// Add the element to the front of the linked list in `O(1)`, unless it has to
    /// allocate, which is `O(chunk_size)`.
    ///
    /// This will not make any allocation unless `len = capacity`, in which case it will
    /// allocate `chunk_size` nodes.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(0, list.capacity());
    /// // add an element, this will cause an allocation
    /// list.push_front(35);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&35), list.front());
    ///
    /// // if we add another, then since the allocation is large enough, this shouldn't
    /// // change the capacity
    /// list.push_front(29);
    /// assert_eq!(list.capacity(), list.chunk_size());
    /// assert_eq!(Some(&29), list.front());
    /// // the first element should still be at the back of the list
    /// assert_eq!(Some(&35), list.back());
    /// ```
    pub fn push_front(&mut self, value: T) {
        let head = self.head;
        let node = self.new_node(head, ptr::null_mut(), value);

        if self.tail.is_null() {
            self.tail = node;
        }
        if !self.head.is_null() {
            unsafe {
                (*self.head).prev = node;
            }
        }

        self.head = node;
        self.len += 1;
    }
    /// Provides a reference to the back element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.back());
    ///
    /// // add an element
    /// list.push_back(32);
    /// assert_eq!(Some(&32), list.back());
    ///
    /// // add another
    /// list.push_back(45);
    /// assert_eq!(Some(&45), list.back());
    ///
    /// // if we add an element in the other end, we still see 45
    /// list.push_front(12);
    /// assert_eq!(Some(&45), list.back());
    /// ```
    #[inline]
    pub fn back(&self) -> Option<&T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&(*self.tail).value) }
        }
    }
    /// Provides a reference to the front element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.front());
    ///
    /// // add an element
    /// list.push_front(32);
    /// assert_eq!(Some(&32), list.front());
    ///
    /// // add another
    /// list.push_front(45);
    /// assert_eq!(Some(&45), list.front());
    ///
    /// // if we add an element in the other end, we still see 45
    /// list.push_back(12);
    /// assert_eq!(Some(&45), list.front());
    /// ```
    #[inline]
    pub fn front(&self) -> Option<&T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&(*self.head).value) }
        }
    }
    /// Provides a mutable reference to the back element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.back_mut());
    ///
    /// // add an element
    /// list.push_back(32);
    ///
    /// // let's change the element we just added
    /// if let Some(back) = list.back_mut() {
    ///     assert_eq!(32, *back);
    ///     *back = 45;
    ///     assert_eq!(45, *back);
    /// }
    /// # else { unreachable!(); }
    ///
    /// // This changed the element in the list.
    /// assert_eq!(Some(&45), list.back());
    /// ```
    #[inline]
    pub fn back_mut(&mut self) -> Option<&mut T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.tail).value) }
        }
    }
    /// Provides a mutable reference to the front element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert_eq!(None, list.front_mut());
    ///
    /// // add an element
    /// list.push_front(32);
    ///
    /// // let's change the element we just added
    /// if let Some(front) = list.front_mut() {
    ///     assert_eq!(32, *front);
    ///     *front = 45;
    ///     assert_eq!(45, *front);
    /// }
    /// # else { unreachable!(); }
    ///
    /// // This changed the element in the list.
    /// assert_eq!(Some(&45), list.front());
    /// ```
    #[inline]
    pub fn front_mut(&mut self) -> Option<&mut T> {
        if self.head.is_null() {
            None
        } else {
            unsafe { Some(&mut (*self.head).value) }
        }
    }
    /// Removes the back element and returns it, or `None` if the list is empty.
    ///
    /// This is an `O(1)` operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// // the list is empty
    /// assert_eq!(None, list.pop_back());
    ///
    /// // add some elements
    /// list.push_back(3);
    /// list.push_back(2);
    /// list.push_back(1);
    /// // other end too
    /// list.push_front(4);
    ///
    /// assert_eq!(4, list.len());
    ///
    /// // let's pop them
    /// assert_eq!(Some(1), list.pop_back());
    /// assert_eq!(Some(2), list.pop_back());
    /// assert_eq!(Some(3), list.pop_back());
    /// assert_eq!(Some(4), list.pop_back());
    /// // we removed all the items
    /// assert_eq!(None, list.pop_back());
    ///
    /// assert_eq!(0, list.len());
    /// ```
    pub fn pop_back(&mut self) -> Option<T> {
        if self.tail.is_null() {
            None
        } else {
            unsafe {
                let tail = self.tail;
                self.tail = (*tail).prev;

                if self.head == tail {
                    self.head = ptr::null_mut();
                }

                self.len -= 1;

                let value = ptr::read(&(*tail).value);
                self.discard_node(tail);
                Some(value)
            }
        }
    }
    /// Removes the front element and returns it, or `None` if the list is empty.
    ///
    /// This is an `O(1)` operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// // the list is empty
    /// assert_eq!(None, list.pop_front());
    ///
    /// // add some elements
    /// list.push_front(2);
    /// list.push_front(1);
    /// // other end too
    /// list.push_back(3);
    /// list.push_back(4);
    ///
    /// assert_eq!(4, list.len());
    ///
    /// // let's pop them
    /// assert_eq!(Some(1), list.pop_front());
    /// assert_eq!(Some(2), list.pop_front());
    /// assert_eq!(Some(3), list.pop_front());
    /// assert_eq!(Some(4), list.pop_front());
    /// // we removed all the items
    /// assert_eq!(None, list.pop_front());
    ///
    /// assert_eq!(0, list.len());
    /// ```
    pub fn pop_front(&mut self) -> Option<T> {
        if self.head.is_null() {
            None
        } else {
            unsafe {
                let head = self.head;
                self.head = (*head).next;

                if self.tail == head {
                    self.tail = ptr::null_mut();
                }

                self.len -= 1;

                let value = ptr::read(&(*head).value);
                self.discard_node(head);
                Some(value)
            }
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`. This
    /// method operates in place and preserves the order of the retained elements.
    ///
    /// If the closure or drop panics then the list is cleared without calling drop and some
    /// capacity may be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.extend(&[0,1,2,3,4,5,6,7,8,9,10]);
    ///
    /// // remove all odd values
    /// list.retain(|&val| val % 2 == 0);
    ///
    /// assert_eq!(list, vec![0,2,4,6,8,10]);
    /// ```
    pub fn retain(&mut self, mut f: impl FnMut(&T) -> bool) {
        self.retain_map(|val| if f(&val) { Some(val) } else { None });
    }
    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements `e` such that `f(&e)` returns `false`. This
    /// method operates in place and preserves the order of the retained elements.
    ///
    /// Note that `retain_mut` lets you mutate every element in the list, regardless of
    /// whether you choose to keep or remove it.
    ///
    /// If the closure or drop panics then the list is cleared without calling drop and
    /// some capacity may be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.extend(&[0,1,2,3,4,5,6,7,8,9,10]);
    ///
    /// // add one to the value, then keep the odd values
    /// list.retain_mut(|val| {
    ///     *val += 1;
    ///     *val % 2 == 1
    /// });
    ///
    /// assert_eq!(list, vec![1,3,5,7,9,11]);
    /// ```
    pub fn retain_mut(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        self.retain_map(|mut val| if f(&mut val) { Some(val) } else { None });
    }
    /// Apply a mapping to the list in place, optionally removing elements.
    ///
    /// This method applies the closure to every element in the list, and replaces it with
    /// the value returned by the closure, or removes it if the closure returned `None`.
    /// This method preserves the order of the retained elements.
    ///
    /// Note that this method allows the closure to take ownership of removed elements.
    ///
    /// If the closure panics then the list is cleared without calling drop and some capacity may
    /// be lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// // Create a list of owned strings.
    /// let mut list: LinkedList<String> = LinkedList::new();
    /// list.extend(vec!["first".to_string(), "second".to_string(), "third".to_string()]);
    ///
    /// // this example removes the middle element and makes the two other uppercase
    /// let mut variable_outside_list = "not second".to_string();
    ///
    /// list.retain_map(|string| {
    ///     if string == "second" {
    ///         // store the element outside the list and remove it
    ///
    ///         variable_outside_list = string;
    ///         None
    ///     } else {
    ///         // replace the element with the uppercase version
    ///
    ///         Some(string.to_uppercase())
    ///     }
    /// });
    ///
    /// assert_eq!(list, vec!["FIRST", "THIRD"]);
    /// assert_eq!(variable_outside_list, "second");
    /// ```
    pub fn retain_map(&mut self, mut f: impl FnMut(T) -> Option<T>) {
        if self.is_empty() {
            return;
        }
        let mut ptr = self.head;
        let mut last_retain: *mut LinkedNode<T> = ptr::null_mut();
        let capacity = self.capacity;

        // If f panics, then we just throw away all the used nodes.
        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;
        // Since we are throwing away the used nodes, then the capacity is decreased by
        // the number of used nodes.
        self.capacity -= self.len;
        // This means that if f panics, then we won't call drop on the remaining values,
        // but that's safe so it's ok.
        // We still deallocate the memory the nodes are stored in when the list is
        // dropped, since we didn't touch the allocations array.

        let mut new_head = ptr::null_mut();
        let mut retained = 0;

        unsafe {
            while !ptr.is_null() {
                let value_ptr = &mut (*ptr).value as *mut T;
                let next_ptr = (*ptr).next;
                match f(ptr::read(value_ptr)) {
                    Some(new_value) => {
                        ptr::write(value_ptr, new_value);
                        if last_retain.is_null() {
                            new_head = ptr;
                        } else {
                            (*last_retain).next = ptr;
                        }
                        (*ptr).prev = last_retain;
                        last_retain = ptr;
                        retained += 1;
                    }
                    None => {
                        self.discard_node(ptr);
                    }
                }
                ptr = next_ptr;
            }
        }

        self.head = new_head;
        self.tail = last_retain;
        self.len = retained;
        // we didn't panic so put capacity back at the actual value
        // we didn't allocate or deallocate in this method, so capacity is the same
        self.capacity = capacity;
    }

    /// Moves all elements from `other` to the back of the list.
    ///
    /// This reuses all the nodes from `other` and moves them into `self`. After this
    /// operation, `other` becomes empty.
    /// Excess capacity as well as ownership of allocations in `other` is also moved into
    /// `self`.
    ///
    /// This method guarantees that the capacity in `self` is increased by
    /// `other.capacity()`, and that `other` will have a capacity of zero when this method
    /// returns.
    ///
    /// Moving the nodes from `other` to `self` is `O(1)`, but moving the excess capacity
    /// and the ownership of allocations requires a full iteration through one of them,
    /// meaning it is linear time, although `append` will always iterate through the
    /// shorter one.
    ///
    /// This method is `O(min(excess_capacity) + min(number_of_allocations))`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list_a: LinkedList<u32> = LinkedList::new();
    /// let mut list_b: LinkedList<u32> = LinkedList::new();
    ///
    /// // add elements to both lists
    /// list_a.extend(&[0,1,2,3,4]);
    /// list_b.extend(&[5,6,7,8,9]);
    ///
    /// // remember their capacities before appending
    /// let cap_a = list_a.capacity();
    /// let cap_b = list_b.capacity();
    ///
    /// list_a.append(&mut list_b);
    ///
    /// // check that the elements were moved
    /// assert_eq!(list_a, vec![0,1,2,3,4,5,6,7,8,9]);
    /// assert_eq!(list_b, vec![]);
    ///
    /// // check that the capacity was moved
    /// assert_eq!(cap_a + cap_b, list_a.capacity());
    /// assert_eq!(0, list_b.capacity());
    /// ```
    pub fn append(&mut self, other: &mut LinkedList<T>) {
        if self.is_empty() {
            // just directly move the chain to self
            self.head = other.head;
            self.tail = other.tail;
            self.len = other.len;
        } else if other.is_empty() {
            // do nothing
        } else {
            // both have elements so we append the chain
            unsafe {
                (*self.tail).next = other.head;
                (*other.head).prev = self.tail;
                self.tail = other.tail;
                self.len += other.len;
            }
        }

        // move allocations
        if self.allocations.len() < other.allocations.len() {
            mem::swap(&mut self.allocations, &mut other.allocations);
        }
        // self.allocations is now the longest array
        self.allocations.extend(other.allocations.drain(..));

        // move unused capacity to self, since self now owns the memory
        self.capacity += other.capacity;
        self.combine_unused_nodes(other);

        // other is now empty
        other.head = ptr::null_mut();
        other.tail = ptr::null_mut();
        other.len = 0;
        other.capacity = 0;
        // allocations is emptied by drain
        debug_assert!(other.allocations.is_empty());
        // unused_nodes is moved by combined_unused_nodes
        debug_assert!(other.unused_nodes.is_null());
    }
    fn combine_unused_nodes(&mut self, other: &mut LinkedList<T>) {
        if self.capacity - self.len < other.capacity - other.len {
            mem::swap(&mut self.unused_nodes, &mut other.unused_nodes);
        }
        // self.unused_nodes is now a longer linked list than the one in other
        // let's find the last node in other.unused_nodes
        let mut ptr = other.unused_nodes;
        if ptr.is_null() {
            // other is null, so we moved all unused_nodes with the swap
            return;
        }
        unsafe {
            // iterate to the last node
            while !(*ptr).next.is_null() {
                ptr = (*ptr).next;
            }
            // we now put the unused_nodes in other in front of the ones in self
            (*ptr).next = self.unused_nodes;
            self.unused_nodes = other.unused_nodes;
            other.unused_nodes = ptr::null_mut();
        }
    }

    /// Provides a forward iterator.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(Some(&0), iter.next());
    /// assert_eq!(Some(&1), iter.next());
    /// assert_eq!(Some(&2), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[inline]
    pub fn iter(&self) -> Iter<T> {
        Iter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }
    /// Provides a forward iterator with mutable references.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_back(0);
    /// list.push_back(1);
    /// list.push_back(2);
    ///
    /// for element in list.iter_mut() {
    ///     *element += 10;
    /// }
    ///
    /// let mut iter = list.iter();
    /// assert_eq!(Some(&10), iter.next());
    /// assert_eq!(Some(&11), iter.next());
    /// assert_eq!(Some(&12), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            head: self.head,
            tail: self.tail,
            len: self.len,
            marker: PhantomData,
        }
    }
    /// Provides a cursor to the contents of the linked list, positioned at the back
    /// element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.cursor_ref_back().is_none());
    /// list.push_back(5);
    /// list.push_back(6);
    ///
    /// if let Some(cursor) = list.cursor_ref_back() {
    ///     assert_eq!(&6, cursor.get());
    ///     assert_eq!(Some(&5), cursor.prev().map(|cursor| cursor.get()));
    ///     assert!(cursor.next().is_none());
    /// }
    /// # else { unreachable!(); }
    /// ```
    #[inline]
    pub fn cursor_ref_back(&self) -> Option<CursorRef<T>> {
        if self.tail.is_null() {
            None
        } else {
            Some(CursorRef::create(self.tail, self.len - 1))
        }
    }
    /// Provides a cursor to the contents of the linked list, positioned at the front
    /// element, or `None` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.cursor_ref_front().is_none());
    /// list.push_front(5);
    /// list.push_front(6);
    ///
    /// if let Some(cursor) = list.cursor_ref_front() {
    ///     assert_eq!(&6, cursor.get());
    ///     assert_eq!(Some(&5), cursor.next().map(|cursor| cursor.get()));
    ///     assert!(cursor.prev().is_none());
    /// }
    /// # else { unreachable!(); }
    /// ```
    #[inline]
    pub fn cursor_ref_front(&self) -> Option<CursorRef<T>> {
        if self.head.is_null() {
            None
        } else {
            Some(CursorRef::create(self.head, 0))
        }
    }

    pub fn cursor_mut_back(&mut self) -> Option<CursorMut<T>> {
        if self.tail.is_null() {
            None
        } else {
            let tail = self.tail;
            let len = self.len;
            Some(CursorMut::create(self, tail, len - 1))
        }
    }
    pub fn cursor_mut_front(&mut self) -> Option<CursorMut<T>> {
        if self.head.is_null() {
            None
        } else {
            let head = self.head;
            Some(CursorMut::create(self, head, 0))
        }
    }

    /// Removes all elements from the `LinkedList`. This method guarantees that capacity
    /// is unchanged.
    ///
    /// This is `O(self.len)` unless `T` has no destructor, in which case it's `O(1)`.
    ///
    /// If drop on any element panics, this method won't drop the remaining nodes, but
    /// the list will still be cleared and no capacity is lost.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_front(2);
    /// list.push_front(1);
    /// assert_eq!(2, list.len());
    /// assert_eq!(Some(&1), list.front());
    ///
    /// let capacity_before_clear = list.capacity();
    ///
    /// list.clear();
    /// assert_eq!(0, list.len());
    /// assert_eq!(None, list.front());
    ///
    /// // no allocation was lost
    /// assert_eq!(capacity_before_clear, list.capacity());
    /// ```
    pub fn clear(&mut self) {
        if self.tail.is_null() {
            return;
        }

        let tail = self.tail;

        unsafe {
            // just append unused_nodes to the linked list, and make the result into the
            // new unused_nodes
            (*self.tail).next = self.unused_nodes;
            // unused_nodes is singly linked, so we don't need the other link
            self.unused_nodes = self.head;
        }
        self.head = ptr::null_mut();
        self.tail = ptr::null_mut();
        self.len = 0;

        if mem::needs_drop::<T>() {
            let mut ptr = tail;
            while !ptr.is_null() {
                unsafe {
                    ptr::drop_in_place(&mut (*ptr).value);
                    ptr = (*ptr).prev;
                }
            }
        }
    }

    /// Returns the number of elements the list can hold without allocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(48);
    /// assert_eq!(48, list.capacity());
    /// ```
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    /// Returns the number of items in the list.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    ///
    /// list.push_front(2);
    /// assert_eq!(1, list.len());
    ///
    /// list.push_back(3);
    /// assert_eq!(2, list.len());
    ///
    /// list.pop_front();
    /// assert_eq!(1, list.len());
    /// ```
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    /// Returns `true` if the list is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// assert!(list.is_empty());
    ///
    /// list.push_back(3);
    /// assert!(!list.is_empty());
    ///
    /// list.pop_front();
    /// assert!(list.is_empty());
    /// ```
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Change the size of future allocations. This has no effect on previous allocations.
    ///
    /// When some operation increases the size of the linked list past the capacity, the
    /// linked list will allocate at least `chunk_size` nodes in one allocation.
    ///
    /// # Panics
    ///
    /// This method panics if `chunk_size` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::new();
    /// // default chunk size is 64
    /// assert_eq!(64, list.chunk_size());
    ///
    /// list.set_chunk_size(3);
    /// assert_eq!(3, list.chunk_size());
    ///
    /// // add an element, which allocates 3 nodes
    /// list.push_back(4);
    /// assert_eq!(3, list.capacity());
    /// ```
    #[inline]
    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        assert!(chunk_size > 0);
        self.chunk_size = chunk_size;
    }
    /// Returns the minimum size of future allocations.  See [`set_chunk_size`].
    ///
    /// [`set_chunk_size`]: #method.set_chunk_size
    #[inline]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Reserves capacity for at least `additional` more elements to be inserted in the
    /// list. This method will not reserve less than [`chunk_size`] nodes to avoid
    /// frequent allocations.
    ///
    /// This is `O(allocation_size)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(5);
    /// assert_eq!(5, list.capacity());
    ///
    /// list.push_back(3);
    /// list.reserve(84); // 84 is larger than the default chunk size
    ///
    /// // there's already one element in the list, so it increases the capacity to 85
    /// // the actual size of the allocation is 80, since the previous capacity was 5
    /// assert_eq!(85, list.capacity());
    /// ```
    ///
    /// [`chunk_size`]: #method.chunk_size
    pub fn reserve(&mut self, additional: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= additional {
            return;
        }
        let to_allocate = additional - free_capacity;

        let chunk_size = self.chunk_size;
        if to_allocate < chunk_size {
            self.allocate(chunk_size);
        } else {
            self.allocate(to_allocate);
        }
    }
    /// Reserves capacity for exactly `additional` more elements to be inserted in the
    /// list.
    ///
    /// This is `O(additional)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use wtf_rs::list::LinkedList;
    ///
    /// let mut list: LinkedList<u32> = LinkedList::with_capacity(5);
    /// assert_eq!(5, list.capacity());
    ///
    /// list.push_back(3);
    /// list.reserve_exact(5);
    ///
    /// // there's already one element in the list, so it increases the capacity to 6
    /// // the actual size of the allocation is 1, since the previous capacity was 5
    /// assert_eq!(6, list.capacity());
    /// ```
    ///
    /// [`chunk_size`]: #method.chunk_size
    pub fn reserve_exact(&mut self, additional: usize) {
        let free_capacity = self.capacity() - self.len();
        if free_capacity >= additional {
            return;
        }
        let to_allocate = additional - free_capacity;
        self.allocate(to_allocate);
    }

    fn discard_node(&mut self, node: *mut LinkedNode<T>) {
        unsafe {
            (*node).next = self.unused_nodes;
        }
        self.unused_nodes = node;
    }
    fn new_node(
        &mut self,
        next: *mut LinkedNode<T>,
        prev: *mut LinkedNode<T>,
        value: T,
    ) -> *mut LinkedNode<T> {
        unsafe {
            if self.unused_nodes.is_null() {
                let chunk_size = self.chunk_size;
                self.allocate(chunk_size);
            }
            let node = self.unused_nodes;
            self.unused_nodes = (*node).next;

            ptr::write(node, LinkedNode { next, prev, value });
            node
        }
    }

    fn allocate(&mut self, amount: usize) {
        if amount == 0 {
            return;
        }
        let mut vec = Vec::with_capacity(amount);
        let base = vec.as_mut_ptr();
        let capacity = vec.capacity();
        self.capacity += capacity;

        mem::forget(vec);

        self.allocations.push((base, capacity));

        // add them to the unused_nodes list in reverse order, so they end up in the
        // correct order if lots of elements are added with push_back
        for i in (0..capacity).rev() {
            let ptr = unsafe { base.add(i) };

            unsafe {
                (*ptr).next = self.unused_nodes;
            }
            self.unused_nodes = ptr;
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        unsafe {
            let mut ptr = self.head;
            while !ptr.is_null() {
                ptr::drop_in_place(&mut (*ptr).value);
                ptr = (*ptr).next;
            }

            for &(vecptr, capacity) in &self.allocations {
                let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                drop(vec);
            }
        }
    }
}
impl<T> Default for LinkedList<T> {
    fn default() -> LinkedList<T> {
        LinkedList::new()
    }
}
impl<T: Clone> Clone for LinkedList<T> {
    fn clone(&self) -> LinkedList<T> {
        let mut list = LinkedList::with_capacity(self.len());
        for item in self.iter() {
            list.push_back(item.clone());
        }
        list
    }
    fn clone_from(&mut self, source: &Self) {
        self.clear();
        self.reserve_exact(source.len());
        for item in source.iter() {
            self.push_back(item.clone());
        }
    }
}
impl<T> FromIterator<T> for LinkedList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut list = LinkedList::with_capacity(iter.size_hint().0);
        for item in iter {
            list.push_back(item);
        }
        list
    }
}
impl<T: Eq> Eq for LinkedList<T> {}
impl<T: PartialEq<U>, U> PartialEq<LinkedList<U>> for LinkedList<T> {
    fn eq(&self, other: &LinkedList<U>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T: PartialEq<U>, U> PartialEq<Vec<U>> for LinkedList<T> {
    fn eq(&self, other: &Vec<U>) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T: PartialEq<U>, U> PartialEq<[U]> for LinkedList<T> {
    fn eq(&self, other: &[U]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<'a, T: PartialEq<U>, U> PartialEq<&'a [U]> for LinkedList<T> {
    fn eq(&self, other: &&'a [U]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<'a, T: PartialEq<U>, U> PartialEq<&'a mut [U]> for LinkedList<T> {
    fn eq(&self, other: &&'a mut [U]) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (a, b) in self.iter().zip(other.iter()) {
            if a != b {
                return false;
            }
        }
        true
    }
}
impl<T: Ord> Ord for LinkedList<T> {
    fn cmp(&self, other: &LinkedList<T>) -> Ordering {
        for (a, b) in self.iter().zip(other.iter()) {
            match a.cmp(b) {
                Ordering::Equal => {}
                ordering => {
                    return ordering;
                }
            }
        }
        Ordering::Equal
    }
}
impl<T: PartialOrd<U>, U> PartialOrd<LinkedList<U>> for LinkedList<T> {
    fn partial_cmp(&self, other: &LinkedList<U>) -> Option<Ordering> {
        for (a, b) in self.iter().zip(other.iter()) {
            match a.partial_cmp(b) {
                Some(Ordering::Equal) => {}
                ordering => {
                    return ordering;
                }
            }
        }
        Some(Ordering::Equal)
    }
}
impl<T> Extend<T> for LinkedList<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.reserve(iter.size_hint().0);
        for item in iter {
            self.push_back(item);
        }
    }
}
impl<'a, T: 'a + Copy> Extend<&'a T> for LinkedList<T> {
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.reserve(iter.size_hint().0);
        for item in iter {
            self.push_back(*item);
        }
    }
}
impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> IntoIter<T> {
        let iter = IntoIter {
            head: self.head,
            tail: self.tail,
            len: self.len,
            allocations: unsafe { ptr::read(&self.allocations) },
        };
        mem::forget(self);
        iter
    }
}
impl<'a, T> IntoIterator for &'a LinkedList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}
impl<'a, T> IntoIterator for &'a mut LinkedList<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}
impl<T: Hash> Hash for LinkedList<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for item in self.iter() {
            item.hash(state);
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for LinkedList<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut out = f.debug_list();
        for item in self.iter() {
            out.entry(item);
        }
        out.finish()
    }
}

// serde impls
#[cfg(feature = "serde")]
extern crate serde;
#[cfg(all(feature = "serde", test))]
extern crate serde_json;
#[cfg(feature = "serde")]
use serde::{de::SeqAccess, de::Visitor, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "serde")]
impl<T: Serialize> Serialize for LinkedList<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for e in self.iter() {
            seq.serialize_element(e)?;
        }
        seq.end()
    }
}
#[cfg(feature = "serde")]
struct LinkedListVisitor<T> {
    marker: PhantomData<T>,
}
#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Visitor<'de> for LinkedListVisitor<T> {
    type Value = LinkedList<T>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a sequence")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut list = match seq.size_hint() {
            Some(hint) => LinkedList::with_capacity(hint),
            None => LinkedList::new(),
        };
        while let Some(next) = seq.next_element()? {
            list.push_back(next);
        }
        Ok(list)
    }
}
#[cfg(feature = "serde")]
impl<'de, T: Deserialize<'de>> Deserialize<'de> for LinkedList<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(LinkedListVisitor {
            marker: PhantomData,
        })
    }
}

#[cfg(all(feature = "serde", test))]
mod serde_test {
    use super::*;
    use rand::prelude::*;
    #[test]
    fn serialize() {
        let mut list: LinkedList<u32> = LinkedList::new();
        list.set_chunk_size(328);
        for _ in 0..1028 {
            list.push_back(random());
        }

        let json = serde_json::to_string(&list).unwrap();
        println!("{}", &json);
        let list2: LinkedList<u32> = serde_json::from_str(&json).unwrap();

        assert_eq!(list, list2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use std::fmt::Write;
    #[test]
    fn retain() {
        let mut list: LinkedList<usize> = LinkedList::new();
        for i in 0..16 {
            list.push_back(i);
        }

        let mut rng = thread_rng();

        let mut mask = [false; 16];
        for val in mask.iter_mut() {
            *val = rng.gen();
        }

        list.retain_map(|i| if mask[i] { Some(i + 1) } else { None });

        let nums: Vec<usize> = (0..16).filter(|&i| mask[i]).map(|i| i + 1).collect();

        println!("{:?}", mask);
        for (a, b) in list.into_iter().zip(nums.into_iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn iter_collect_compare() {
        let mut list = LinkedList::new();
        for i in 0..64usize {
            list.push_back(i);
        }
        let list2: LinkedList<u32> = list.iter().map(|&i| i as u32).collect();
        let vec: Vec<u32> = list.into_iter().map(|i| i as u32).collect();

        assert_eq!(list2, vec);
    }
    #[test]
    fn debug_print_list() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);
        write!(output, "{:?}", list).unwrap();
        assert_eq!(output, "[1, 2, 3, 4]");
    }
    #[test]
    fn debug_print_iter() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.iter();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::Iter[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::Iter[2, 3]");
        output.clear();
    }
    #[test]
    fn debug_print_iter_mut() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.iter_mut();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IterMut[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IterMut[2, 3]");
        output.clear();
    }
    #[test]
    fn debug_print_into_iter() {
        let mut output = String::new();
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        let mut iter = list.into_iter();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IntoIter[1, 2, 3, 4]");
        output.clear();

        let _ = iter.next();
        let _ = iter.next_back();

        write!(output, "{:?}", iter).unwrap();
        assert_eq!(output, "LinkedList::IntoIter[2, 3]");
        output.clear();
    }
    #[test]
    fn iter_mut_several_mut_ref() {
        let mut list = LinkedList::new();
        list.push_back(1);
        list.push_back(2);
        list.push_back(3);
        list.push_back(4);

        {
            let mut iter_mut = list.iter_mut();
            let ref1 = iter_mut.next().unwrap();
            let ref2 = iter_mut.next().unwrap();
            drop(iter_mut);
            *ref1 = 6;
            *ref2 = 7;
        }

        assert_eq!(list, vec![6, 7, 3, 4]);
    }
}

pub mod cursor {
    //! This module provides cursors on a linked list, allowing more complicated list
    //! operations.
    use super::*;

    use std::fmt;
    use std::iter::Rev;
    use std::marker::PhantomData;

    /// A cursor with immutable access to the `LinkedList`.
    ///
    /// A `CursorRef` always points to a valid element in a linked list, and allows immutable
    /// access with the [`get`] method. The cursor allows moving around the `LinkedList`
    /// in both directions and is created using the [`cursor_ref_front`] and
    /// [`cursor_ref_back`] methods.
    ///
    /// A cursor is simply a pointer, and is therefore `Copy`, allowing duplicating a cursor
    /// to some element.
    ///
    /// [`get`]: #method.get
    /// [`cursor_ref_front`]: struct.LinkedList.html#method.cursor_ref_front
    /// [`cursor_ref_back`]: struct.LinkedList.html#method.cursor_ref_back
    pub struct CursorRef<'a, T: 'a> {
        cursor: *const LinkedNode<T>,
        index: usize,
        marker: PhantomData<&'a T>,
    }

    impl<'a, T> CursorRef<'a, T> {
        pub(crate) fn create(cursor: *const LinkedNode<T>, index: usize) -> Self {
            CursorRef {
                cursor,
                index,
                marker: PhantomData,
            }
        }
        /// Returns the next cursor, or `None` if this is the back of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// let front = list.cursor_ref_front().unwrap();
        /// let back = list.cursor_ref_back().unwrap();
        ///
        /// assert!(back.next().is_none());
        /// assert!(front.next().unwrap().ptr_eq(back));
        /// ```
        pub fn next(self) -> Option<CursorRef<'a, T>> {
            let next = unsafe { (*self.cursor).next };
            if next.is_null() {
                None
            } else {
                Some(CursorRef::create(next, self.index + 1))
            }
        }
        /// Returns the previous cursor, or `None` if this is the front of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// let front = list.cursor_ref_front().unwrap();
        /// let back = list.cursor_ref_back().unwrap();
        ///
        /// assert!(front.prev().is_none());
        /// assert!(back.prev().unwrap().ptr_eq(front));
        /// ```
        pub fn prev(self) -> Option<CursorRef<'a, T>> {
            let prev = unsafe { (*self.cursor).prev };
            if prev.is_null() {
                None
            } else {
                Some(CursorRef::create(prev, self.index - 1))
            }
        }
        /// Provides a immutable reference to the element this cursor currently points at. The
        /// reference is bound to the list and can outlive the cursor.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// if let Some(cursor) = list.cursor_ref_front() {
        ///     assert_eq!(&1, cursor.get());
        ///     assert_eq!(&2, cursor.next().unwrap().get());
        /// }
        /// # else { unreachable!(); }
        /// ```
        pub fn get(self) -> &'a T {
            unsafe { &(*self.cursor).value }
        }
        /// Returns the index of the cursor in the linked list. The front of the list has
        /// index zero and the back of the list has index `len - 1`.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        /// list.push_back(4);
        ///
        /// if let Some(front) = list.cursor_ref_front() {
        ///     assert_eq!(0, front.index());
        ///     assert_eq!(1, front.next().unwrap().index());
        /// }
        /// # else { unreachable!(); }
        /// if let Some(back) = list.cursor_ref_back() {
        ///     assert_eq!(list.len() - 1, back.index());
        ///     assert_eq!(list.len() - 2, back.prev().unwrap().index());
        /// }
        /// # else { unreachable!(); }
        /// ```
        pub fn index(self) -> usize {
            self.index
        }
        /// Returns `true` if the cursor points to the front of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// assert!(list.cursor_ref_front().unwrap().is_front());
        /// assert!(!list.cursor_ref_back().unwrap().is_front());
        /// ```
        pub fn is_front(&self) -> bool {
            unsafe { (*self.cursor).prev.is_null() }
        }
        /// Returns `true` if the cursor points to the back of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// assert!(!list.cursor_ref_front().unwrap().is_back());
        /// assert!(list.cursor_ref_back().unwrap().is_back());
        /// ```
        pub fn is_back(&self) -> bool {
            unsafe { (*self.cursor).next.is_null() }
        }
        /// Return `true` if the cursors point to the same element. Note that this does not
        /// compare the actual values they point to. Returns `false` if the cursors are from
        /// different `LinkedList`s, even if their `index` is equal.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1); // front
        /// list.push_back(2); // middle
        /// list.push_back(3); // back
        ///
        /// let front = list.cursor_ref_front().unwrap();
        /// let back = list.cursor_ref_back().unwrap();
        /// assert!(!front.ptr_eq(back));
        ///
        /// let middle = front.next().unwrap();
        /// assert!(middle.ptr_eq(back.prev().unwrap()));
        ///
        /// assert!(middle.next().unwrap().ptr_eq(back));
        ///
        /// let mut other_list: LinkedList<u32> = LinkedList::new();
        /// other_list.push_back(1);
        /// other_list.push_back(2);
        /// other_list.push_back(3);
        /// assert!(!back.ptr_eq(other_list.cursor_ref_back().unwrap()));
        /// ```
        pub fn ptr_eq(self, other: CursorRef<T>) -> bool {
            self.cursor == other.cursor
        }
    }
    impl<'a, T> Clone for CursorRef<'a, T> {
        fn clone(&self) -> Self {
            CursorRef {
                cursor: self.cursor,
                index: self.index,
                marker: PhantomData,
            }
        }
    }
    impl<'a, T> Copy for CursorRef<'a, T> {}
    unsafe impl<'a, T: Sync> Send for CursorRef<'a, T> {}
    unsafe impl<'a, T: Sync> Sync for CursorRef<'a, T> {}
    impl<'a, T: fmt::Debug> fmt::Debug for CursorRef<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            f.debug_tuple("CursorRef").field(self.get()).finish()
        }
    }

    /// An unique cursor with mutable access to the `LinkedList`.
    ///
    /// A `CursorMut` always points to a valid element in a linked list, and allows mutable
    /// access with the [`get`] method. The cursor allows moving around the `LinkedList`
    /// in both directions and is created using the [`cursor_mut_front`] and
    /// [`cursor_mut_back`] methods.
    ///
    /// [`get`]: #method.get
    /// [`cursor_mut_front`]: struct.LinkedList.html#method.cursor_mut_front
    /// [`cursor_mut_back`]: struct.LinkedList.html#method.cursor_mut_back
    pub struct CursorMut<'a, T: 'a> {
        list: &'a mut LinkedList<T>,
        cursor: *mut LinkedNode<T>,
        index: usize,
    }

    impl<'a, T> CursorMut<'a, T> {
        pub(crate) fn create(
            list: &'a mut LinkedList<T>,
            cursor: *mut LinkedNode<T>,
            index: usize,
        ) -> Self {
            CursorMut {
                list,
                cursor,
                index,
            }
        }
        /// Move the cursor to the next element, unless it's the back element of the list.
        /// Returns `true` if the cursor was moved.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// // we can't have two mutable cursors at the same time
        /// {
        ///     let mut front = list.cursor_mut_front().unwrap();
        ///     assert!(front.go_next());  // first go_next succeeds
        ///     assert!(!front.go_next()); // second go_next fails
        /// }
        /// {
        ///     let mut back = list.cursor_mut_back().unwrap();
        ///     assert!(!back.go_next()); // go_next fails
        /// }
        /// ```
        pub fn go_next(&mut self) -> bool {
            let next = unsafe { (*self.cursor).next };
            if next.is_null() {
                false
            } else {
                self.cursor = next;
                self.index += 1;
                true
            }
        }
        /// Consume this cursor and return the next cursor, unless this is the back of the
        /// list, in which case `None` is returned.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// // we can't have two mutable cursors at the same time
        /// {
        ///     let mut front = list.cursor_mut_front().unwrap();
        ///     assert!(front.next().is_some());
        /// }
        /// {
        ///     let mut back = list.cursor_mut_back().unwrap();
        ///     assert!(back.next().is_none());
        /// }
        /// ```
        pub fn next(mut self) -> Option<CursorMut<'a, T>> {
            if self.go_next() {
                Some(self)
            } else {
                None
            }
        }
        /// Move the cursor to the previous element, unless it's the front element of the
        /// list.  Returns `true` if the cursor was moved.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// // we can't have two mutable cursors at the same time
        /// {
        ///     let mut front = list.cursor_mut_back().unwrap();
        ///     assert!(front.go_prev());  // first go_prev succeeds
        ///     assert!(!front.go_prev()); // second go_prev fails
        /// }
        /// {
        ///     let mut back = list.cursor_mut_front().unwrap();
        ///     assert!(!back.go_prev()); // go_prev fails
        /// }
        /// ```
        pub fn go_prev(&mut self) -> bool {
            let prev = unsafe { (*self.cursor).prev };
            if prev.is_null() {
                false
            } else {
                self.cursor = prev;
                self.index -= 1;
                true
            }
        }
        /// Consume this cursor and return the previous cursor, unless this is the front of
        /// the list, in which case `None` is returned.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(5);
        /// list.push_back(6);
        ///
        /// // we can't have two mutable cursors at the same time
        /// {
        ///     let mut front = list.cursor_mut_front().unwrap();
        ///     assert!(front.prev().is_none());
        /// }
        /// {
        ///     let mut back = list.cursor_mut_back().unwrap();
        ///     assert!(back.prev().is_some());
        /// }
        /// ```
        pub fn prev(mut self) -> Option<CursorMut<'a, T>> {
            if self.go_prev() {
                Some(self)
            } else {
                None
            }
        }

        /// Insert a new node into the linked list. This method does not move the cursor, and
        /// the newly created element will be the next element when it returns.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(3);
        ///
        /// if let Some(mut front) = list.cursor_mut_front() {
        ///     assert_eq!(&1, front.get_ref());
        ///#    assert_eq!(0, front.index());
        ///     front.insert_next(2);
        ///#    assert_eq!(0, front.index());
        ///     assert_eq!(&1, front.get_ref());
        ///     assert_eq!(&2, front.next().unwrap().get_ref());
        /// }
        ///# else { unreachable!(); }
        /// assert_eq!(list, vec![1, 2, 3]);
        /// ```
        pub fn insert_next(&mut self, value: T) {
            let nextnext = unsafe { (*self.cursor).next };
            let node = self.list.new_node(nextnext, self.cursor, value);
            self.list.len += 1;

            unsafe {
                (*self.cursor).next = node;
                if nextnext.is_null() {
                    self.list.tail = node;
                } else {
                    (*nextnext).prev = node;
                }
            }
        }
        /// Insert a new node into the linked list. This method does not move the cursor, and
        /// the newly created element will be the previous element when it returns.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(mut front) = list.cursor_mut_front() {
        ///     assert_eq!(&2, front.get_ref());
        ///#    assert_eq!(0, front.index());
        ///     front.insert_prev(1);
        ///#    assert_eq!(1, front.index());
        ///     assert_eq!(&2, front.get_ref());
        ///     assert_eq!(&1, front.prev().unwrap().get_ref());
        /// }
        ///# else { unreachable!(); }
        /// assert_eq!(list, vec![1, 2, 3]);
        /// ```
        pub fn insert_prev(&mut self, value: T) {
            let prevprev = unsafe { (*self.cursor).prev };
            let node = self.list.new_node(self.cursor, prevprev, value);
            self.index += 1;
            self.list.len += 1;

            unsafe {
                (*self.cursor).prev = node;
                if prevprev.is_null() {
                    self.list.head = node;
                } else {
                    (*prevprev).next = node;
                }
            }
        }

        /// Remove the value and consume the cursor.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// for i in 0..8 {
        ///     list.push_back(i*i);
        /// }
        ///
        /// // let's remove the value at index 4
        /// if let Some(mut cursor) = list.cursor_mut_front() {
        ///     while cursor.index() != 4 {
        ///         assert!(cursor.go_next());
        ///     }
        ///     assert_eq!(cursor.remove(), 16);
        /// }
        /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
        /// ```
        pub fn remove(self) -> T {
            unsafe {
                let prev = (*self.cursor).prev;
                let next = (*self.cursor).next;

                if prev.is_null() {
                    self.list.head = next;
                } else {
                    (*prev).next = next;
                }

                if next.is_null() {
                    self.list.tail = prev;
                } else {
                    (*next).prev = prev;
                }

                let value = ptr::read(&(*self.cursor).value);
                self.list.discard_node(self.cursor);
                self.list.len -= 1;
                value
            }
        }
        /// Remove the value and return the cursor to the next element, or `None` if this is
        /// the back.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// for i in 0..8 {
        ///     list.push_back(i*i);
        /// }
        ///
        /// // let's remove the value at index 4
        /// if let Some(mut cursor) = list.cursor_mut_front() {
        ///     while cursor.index() != 4 {
        ///         assert!(cursor.go_next());
        ///     }
        ///#    assert_eq!(4, cursor.index());
        ///     if let (removed, Some(next)) = cursor.remove_go_next() {
        ///#        assert_eq!(4, next.index());
        ///         assert_eq!(16, removed);
        ///         assert_eq!(&25, next.get_ref());
        ///         assert_eq!(&36, next.next().unwrap().get_ref());
        ///     }
        ///#    else { unreachable!(); }
        /// }
        ///# else { unreachable!(); }
        ///
        /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
        /// ```
        pub fn remove_go_next(self) -> (T, Option<CursorMut<'a, T>>) {
            unsafe {
                let cursor = self.cursor;
                let prev = (*cursor).prev;
                let next = (*cursor).next;

                if prev.is_null() {
                    self.list.head = next;
                } else {
                    (*prev).next = next;
                }

                if next.is_null() {
                    self.list.tail = prev;
                } else {
                    (*next).prev = prev;
                }

                let value = ptr::read(&(*cursor).value);
                self.list.discard_node(cursor);
                self.list.len -= 1;
                if next.is_null() {
                    (value, None)
                } else {
                    (value, Some(CursorMut::create(self.list, next, self.index)))
                }
            }
        }
        /// Remove the value and return the cursor to the previous element, or `None` if this
        /// is the front.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// for i in 0..8 {
        ///     list.push_back(i*i);
        /// }
        ///
        /// // let's remove the value at index 4
        /// if let Some(mut cursor) = list.cursor_mut_front() {
        ///     while cursor.index() != 4 {
        ///         assert!(cursor.go_next());
        ///     }
        ///#    assert_eq!(4, cursor.index());
        ///     if let (removed, Some(next)) = cursor.remove_go_prev() {
        ///#        assert_eq!(3, next.index());
        ///         assert_eq!(16, removed);
        ///         assert_eq!(&9, next.get_ref());
        ///         assert_eq!(&25, next.next().unwrap().get_ref());
        ///     }
        ///#    else { unreachable!(); }
        /// }
        ///# else { unreachable!(); }
        ///
        /// assert_eq!(list, vec![0, 1, 4, 9, 25, 36, 49]);
        /// ```
        pub fn remove_go_prev(self) -> (T, Option<CursorMut<'a, T>>) {
            unsafe {
                let cursor = self.cursor;
                let prev = (*cursor).prev;
                let next = (*cursor).next;

                if prev.is_null() {
                    self.list.head = next;
                } else {
                    (*prev).next = next;
                }

                if next.is_null() {
                    self.list.tail = prev;
                } else {
                    (*next).prev = prev;
                }

                let value = ptr::read(&(*cursor).value);
                self.list.discard_node(cursor);
                self.list.len -= 1;
                if next.is_null() {
                    (value, None)
                } else {
                    (
                        value,
                        Some(CursorMut::create(self.list, prev, self.index - 1)),
                    )
                }
            }
        }

        /// Swap the current value for a new value.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(mut cursor) = list.cursor_mut_front() {
        ///     assert_eq!(cursor.swap(5), 1);
        ///     cursor.go_next();
        ///     assert_eq!(cursor.swap(8), 2);
        ///     assert_eq!(cursor.swap(3), 8);
        ///     cursor.go_next();
        ///     assert_eq!(cursor.swap(100), 3);
        /// }
        ///# else { unreachable!(); }
        ///
        /// assert_eq!(list, vec![5, 3, 100]);
        /// ```
        pub fn swap(&mut self, value: T) -> T {
            unsafe {
                let previous_value = ptr::read(&(*self.cursor).value);
                ptr::write(&mut (*self.cursor).value, value);
                previous_value
            }
        }

        /// Provides a mutable reference to the element this cursor currently points at.
        ///
        /// # Safety
        ///
        /// While this method only allows one mutable reference to exist at a time, you can
        /// cast it to a mutable raw pointer, which will allow you to create several mutable
        /// pointers into the list.
        ///
        /// The pointer is valid until the node is removed from the list or the list is
        /// dropped. The pointer will remain valid if the node is moved to a different linked
        /// list using [`append`].
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// if let Some(mut cursor) = list.cursor_mut_front() {
        ///     *cursor.get() = 3;
        /// }
        /// # else { unreachable!(); }
        ///
        /// assert_eq!(list, vec![3, 2]);
        /// ```
        ///
        /// [`append`]: struct.LinkedList.html#method.append
        #[allow(unknown_lints)]
        #[allow(clippy::needless_lifetimes)]
        pub fn get<'cursor>(&'cursor mut self) -> &'cursor mut T {
            unsafe { &mut (*self.cursor).value }
        }
        /// Provides an immutable reference to the element this cursor currently points at.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// if let Some(cursor) = list.cursor_mut_front() {
        ///     assert_eq!(&1, cursor.get_ref());
        ///     assert_eq!(&2, cursor.next().unwrap().get_ref());
        /// }
        /// # else { unreachable!(); }
        /// ```
        #[allow(unknown_lints)]
        #[allow(clippy::needless_lifetimes)]
        pub fn get_ref<'cursor>(&'cursor self) -> &'cursor T {
            unsafe { &(*self.cursor).value }
        }
        /// Consume the cursor and return a mutable reference.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// {
        ///     // this reference outlives the cursor
        ///     let reference = list.cursor_mut_front().unwrap().into_mut();
        ///     assert_eq!(*reference, 1);
        ///     *reference = 5;
        /// }
        /// assert_eq!(Some(&5), list.front());
        /// ```
        pub fn into_mut(self) -> &'a mut T {
            unsafe { &mut (*self.cursor).value }
        }
        /// Returns the index of the cursor in the linked list. The front of the list has
        /// index zero and the back of the list has index `len - 1`.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        /// list.push_back(4);
        ///
        /// if let Some(front) = list.cursor_mut_front() {
        ///     assert_eq!(0, front.index());
        ///     assert_eq!(1, front.next().unwrap().index());
        /// }
        /// # else { unreachable!(); }
        /// if let Some(back) = list.cursor_mut_back() {
        ///     assert_eq!(3, back.index());
        ///     assert_eq!(2, back.prev().unwrap().index());
        /// }
        /// # else { unreachable!(); }
        /// ```
        pub fn index(&self) -> usize {
            self.index
        }
        /// Returns `true` if the cursor points to the front of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// assert!(list.cursor_mut_front().unwrap().is_front());
        /// assert!(!list.cursor_mut_back().unwrap().is_front());
        /// ```
        pub fn is_front(&self) -> bool {
            unsafe { (*self.cursor).prev.is_null() }
        }
        /// Returns `true` if the cursor points to the back of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        ///
        /// assert!(!list.cursor_mut_front().unwrap().is_back());
        /// assert!(list.cursor_mut_back().unwrap().is_back());
        /// ```
        pub fn is_back(&self) -> bool {
            unsafe { (*self.cursor).next.is_null() }
        }

        /// Return an iterator from this element to the tail of the list.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(head) = list.cursor_mut_front() {
        ///     let iter_from_2 = head.next().unwrap().iter_to_tail();
        ///     let vec: Vec<u32> = iter_from_2.map(|&mut v| v).collect();
        ///     assert_eq!(vec, [2, 3]);
        /// }
        ///# else { unreachable!(); }
        /// ```
        pub fn iter_to_tail(self) -> IterMut<'a, T> {
            let len = self.list.len - self.index;
            IterMut {
                head: self.cursor,
                tail: self.list.tail,
                marker: PhantomData,
                len,
            }
        }
        /// Return an iterator from the tail of the list to this element (inclusive). This is
        /// the same as calling `cursor.iter_to_tail().rev()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(head) = list.cursor_mut_front() {
        ///     let iter_to_2 = head.next().unwrap().iter_from_tail();
        ///     let vec: Vec<u32> = iter_to_2.map(|&mut v| v).collect();
        ///     assert_eq!(vec, [3, 2]);
        /// }
        ///# else { unreachable!(); }
        /// ```
        pub fn iter_from_tail(self) -> Rev<IterMut<'a, T>> {
            self.iter_to_tail().rev()
        }
        /// Return an iterator from this element to the head of the list. This is the same as
        /// calling `cursor.iter_from_head().rev()`.
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(head) = list.cursor_mut_front() {
        ///     let iter_from_2 = head.next().unwrap().iter_to_head();
        ///     let vec: Vec<u32> = iter_from_2.map(|&mut v| v).collect();
        ///     assert_eq!(vec, [2, 1]);
        /// }
        ///# else { unreachable!(); }
        /// ```
        pub fn iter_to_head(self) -> Rev<IterMut<'a, T>> {
            self.iter_from_head().rev()
        }
        /// Return an iterator from the head of the list to this element (inclusive).
        ///
        /// # Examples
        ///
        /// ```
        /// use wtf_rs::list::LinkedList;
        ///
        /// let mut list: LinkedList<u32> = LinkedList::new();
        /// list.push_back(1);
        /// list.push_back(2);
        /// list.push_back(3);
        ///
        /// if let Some(head) = list.cursor_mut_front() {
        ///     let iter_to_2 = head.next().unwrap().iter_from_head();
        ///     let vec: Vec<u32> = iter_to_2.map(|&mut v| v).collect();
        ///     assert_eq!(vec, [1, 2]);
        /// }
        ///# else { unreachable!(); }
        /// ```
        pub fn iter_from_head(self) -> IterMut<'a, T> {
            IterMut {
                head: self.list.head,
                tail: self.cursor,
                len: self.index + 1,
                marker: PhantomData,
            }
        }
    }
    unsafe impl<'a, T: Send> Send for CursorMut<'a, T> {}
    unsafe impl<'a, T: Sync> Sync for CursorMut<'a, T> {}
    impl<'a, T: fmt::Debug> fmt::Debug for CursorMut<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            f.debug_tuple("CursorMut").field(self.get_ref()).finish()
        }
    }
}

pub mod iter {
    //! This module provides various iterators over the linked list.
    use super::*;

    use std::fmt;
    #[cfg(feature = "nightly")]
    use std::iter::TrustedLen;
    use std::iter::{DoubleEndedIterator, ExactSizeIterator, FusedIterator};
    use std::marker::PhantomData;
    use std::ptr;

    /// An iterator over borrowed values from a linked list.
    pub struct Iter<'a, T: 'a> {
        pub(crate) head: *const LinkedNode<T>,
        pub(crate) tail: *const LinkedNode<T>,
        pub(crate) len: usize,
        pub(crate) marker: PhantomData<&'a T>,
    }
    #[cfg(feature = "nightly")]
    unsafe impl<'a, T> TrustedLen for Iter<'a, T> {}
    unsafe impl<'a, T: Sync> Send for Iter<'a, T> {}
    unsafe impl<'a, T: Sync> Sync for Iter<'a, T> {}
    impl<'a, T> Iterator for Iter<'a, T> {
        type Item = &'a T;
        fn next(&mut self) -> Option<&'a T> {
            if self.len > 0 {
                debug_assert!(!self.head.is_null());
                unsafe {
                    let value = Some(&(*self.head).value);
                    self.head = (*self.head).next;
                    self.len -= 1;
                    value
                }
            } else {
                None
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.len, Some(self.len))
        }
        fn count(self) -> usize {
            self.len
        }
        fn last(self) -> Option<&'a T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe { Some(&(*self.tail).value) }
            } else {
                None
            }
        }
    }
    impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
        fn next_back(&mut self) -> Option<&'a T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe {
                    let value = Some(&(*self.tail).value);
                    self.tail = (*self.tail).prev;
                    self.len -= 1;
                    value
                }
            } else {
                None
            }
        }
    }
    impl<'a, T> FusedIterator for Iter<'a, T> {}
    impl<'a, T> ExactSizeIterator for Iter<'a, T> {
        fn len(&self) -> usize {
            self.len
        }
    }
    impl<'a, T> Clone for Iter<'a, T> {
        fn clone(&self) -> Self {
            Iter {
                head: self.head,
                tail: self.tail,
                len: self.len,
                marker: self.marker,
            }
        }
    }
    impl<'a, T> Copy for Iter<'a, T> {}
    impl<'a, T: fmt::Debug> fmt::Debug for Iter<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            f.write_str("LinkedList::Iter")?;
            let mut out = f.debug_list();
            let mut ptr = self.head;
            for _ in 0..self.len {
                unsafe {
                    out.entry(&(*ptr).value);
                    ptr = (*ptr).next;
                }
            }
            out.finish()
        }
    }

    /// An iterator over mutably borrowed values from a linked list.
    pub struct IterMut<'a, T: 'a> {
        pub(crate) head: *mut LinkedNode<T>,
        pub(crate) tail: *mut LinkedNode<T>,
        pub(crate) len: usize,
        pub(crate) marker: PhantomData<&'a mut T>,
    }
    #[cfg(feature = "nightly")]
    unsafe impl<'a, T> TrustedLen for IterMut<'a, T> {}
    unsafe impl<'a, T: Send> Send for IterMut<'a, T> {}
    unsafe impl<'a, T: Sync> Sync for IterMut<'a, T> {}
    impl<'a, T> Iterator for IterMut<'a, T> {
        type Item = &'a mut T;
        fn next(&mut self) -> Option<&'a mut T> {
            if self.len > 0 {
                debug_assert!(!self.head.is_null());
                unsafe {
                    let value = Some(&mut (*self.head).value);
                    self.head = (*self.head).next;
                    self.len -= 1;
                    value
                }
            } else {
                None
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.len, Some(self.len))
        }
        fn count(self) -> usize {
            self.len
        }
        fn last(self) -> Option<&'a mut T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe { Some(&mut (*self.tail).value) }
            } else {
                None
            }
        }
    }
    impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
        fn next_back(&mut self) -> Option<&'a mut T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe {
                    let value = Some(&mut (*self.tail).value);
                    self.tail = (*self.tail).prev;
                    self.len -= 1;
                    value
                }
            } else {
                None
            }
        }
    }
    impl<'a, T> FusedIterator for IterMut<'a, T> {}
    impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
        fn len(&self) -> usize {
            self.len
        }
    }
    impl<'a, T: fmt::Debug> fmt::Debug for IterMut<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            f.write_str("LinkedList::IterMut")?;
            let mut out = f.debug_list();
            let mut ptr = self.head;
            for _ in 0..self.len {
                unsafe {
                    // creating this reference cannot alias with any mutable reference
                    // returned by the iterator, since it only prints the values not yet
                    // returned
                    out.entry(&(*ptr).value);
                    ptr = (*ptr).next;
                }
            }
            out.finish()
        }
    }

    /// An iterator over values from a linked list.
    pub struct IntoIter<T> {
        pub(crate) head: *mut LinkedNode<T>,
        pub(crate) tail: *mut LinkedNode<T>,
        pub(crate) len: usize,
        pub(crate) allocations: Vec<(*mut LinkedNode<T>, usize)>,
    }
    #[cfg(feature = "nightly")]
    unsafe impl<T> TrustedLen for IntoIter<T> {}
    unsafe impl<T: Send> Send for IntoIter<T> {}
    unsafe impl<T: Sync> Sync for IntoIter<T> {}
    impl<T> Iterator for IntoIter<T> {
        type Item = T;
        fn next(&mut self) -> Option<T> {
            if self.len > 0 {
                debug_assert!(!self.head.is_null());
                unsafe {
                    let value = ptr::read(&(*self.head).value);
                    self.head = (*self.head).next;
                    self.len -= 1;
                    Some(value)
                }
            } else {
                None
            }
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.len, Some(self.len))
        }
        fn count(self) -> usize {
            self.len
        }
        fn last(self) -> Option<T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe { Some(ptr::read(&(*self.tail).value)) }
            } else {
                None
            }
        }
    }
    impl<T> DoubleEndedIterator for IntoIter<T> {
        fn next_back(&mut self) -> Option<T> {
            if self.len > 0 {
                debug_assert!(!self.tail.is_null());
                unsafe {
                    let value = ptr::read(&(*self.tail).value);
                    self.tail = (*self.tail).prev;
                    self.len -= 1;
                    Some(value)
                }
            } else {
                None
            }
        }
    }
    impl<T> FusedIterator for IntoIter<T> {}
    impl<T> ExactSizeIterator for IntoIter<T> {
        fn len(&self) -> usize {
            self.len
        }
    }
    impl<T> Drop for IntoIter<T> {
        fn drop(&mut self) {
            unsafe {
                // drop remaining elements
                while let Some(_) = self.next() {}

                // deallocate memory
                for &(vecptr, capacity) in &self.allocations {
                    let vec = Vec::from_raw_parts(vecptr, 0, capacity);
                    drop(vec);
                }
            }
        }
    }
    impl<T: fmt::Debug> fmt::Debug for IntoIter<T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
            f.write_str("LinkedList::IntoIter")?;
            let mut out = f.debug_list();
            let mut ptr = self.head;
            for _ in 0..self.len {
                unsafe {
                    out.entry(&(*ptr).value);
                    ptr = (*ptr).next;
                }
            }
            out.finish()
        }
    }
}
