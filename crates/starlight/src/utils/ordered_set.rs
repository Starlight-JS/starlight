use std::ops::Deref;

/*use core::borrow::Borrow;
use core::convert::TryFrom;
use core::fmt::Debug;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use core::iter::FromIterator;
use core::ops::Deref;
use core::slice::Iter;

#[derive(Clone, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct OrderedSet<T>(Vec<T>);

impl<T> OrderedSet<T> {
    /// Creates a new `OrderedSet`.
    #[inline]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates a new `OrderedSet` with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    /// Returns the number of elements in the `OrderedSet`.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the `OrderedSet` contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the slice of elements.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    /// Returns the first element in the set, or `None` if the set is empty.
    #[inline]
    pub fn head(&self) -> Option<&T> {
        self.0.first()
    }

    /// Returns a mutable referece to the first element in the set, or `None` if
    /// the set is empty.
    #[inline]
    pub fn head_mut(&mut self) -> Option<&mut T> {
        self.0.first_mut()
    }

    /// Returns the last element in the set, or `None` if the set is empty.
    #[inline]
    pub fn tail(&self) -> Option<&T> {
        self.0.last()
    }

    /// Returns a mutable referece the last element in the set, or `None` if the
    /// set is empty.
    #[inline]
    pub fn tail_mut(&mut self) -> Option<&mut T> {
        self.0.last_mut()
    }

    /// Returns a slice containing all elements in the `OrderedSet`.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    /// Consumes the `OrderedSet` and returns the elements as a `Vec<T>`.
    #[inline]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    /// Clears the `OrderedSet`, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Returns `true` if the `OrderedSet` contains the given value.
    pub fn contains<U>(&self, item: &U) -> bool
    where
        T: AsRef<U>,
        U: PartialEq + ?Sized,
    {
        self.0.iter().any(|other| other.as_ref() == item)
    }

    /// Adds a new value to the end of the `OrderedSet`; returns `true` if the
    /// value was successfully added.
    pub fn append(&mut self, item: T) -> bool
    where
        T: PartialEq,
    {
        if self.0.contains(&item) {
            false
        } else {
            self.0.push(item);
            true
        }
    }

    /// Adds a new value to the start of the `OrderedSet`; returns `true` if the
    /// value was successfully added.
    pub fn prepend(&mut self, item: T) -> bool
    where
        T: PartialEq,
    {
        if self.0.contains(&item) {
            false
        } else {
            self.0.insert(0, item);
            true
        }
    }

    /// Replaces a `current` value with the given `update` value; returns `true`
    /// if the value was successfully replaced.
    #[inline]
    pub fn replace<U>(&mut self, current: &U, update: T) -> bool
    where
        T: PartialEq + Borrow<U>,
        U: PartialEq + ?Sized,
    {
        self.change(update, |item, update| {
            item.borrow() == current || item == update
        })
    }

    /// Updates an existing value in the `OrderedSet`; returns `true` if the value
    /// was successfully updated.
    #[inline]
    pub fn update(&mut self, update: T) -> bool
    where
        T: PartialEq,
    {
        self.change(update, |item, update| item == update)
    }

    /// Removes all matching items from the set.
    #[inline]
    pub fn remove<U>(&mut self, item: &U)
    where
        T: PartialEq + Borrow<U>,
        U: PartialEq + ?Sized,
    {
        self.0.retain(|this| this.borrow() != item);
    }
    pub fn retain(&mut self, f: impl FnMut(&T) -> bool) {
        self.0.retain(f);
    }
    fn change<F>(&mut self, data: T, f: F) -> bool
    where
        F: Fn(&T, &T) -> bool,
    {
        let index: Option<usize> = self.0.iter().position(|item| f(item, &data));

        if let Some(index) = index {
            let keep: Vec<T> = self
                .0
                .drain(index..)
                .filter(|item| !f(item, &data))
                .collect();

            self.0.extend(keep);
            self.0.insert(index, data);
        }

        index.is_some()
    }
}

impl<T> Debug for OrderedSet<T>
where
    T: Debug,
{
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_set().entries(self.iter()).finish()
    }
}

impl<T> Deref for OrderedSet<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Default for OrderedSet<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> FromIterator<T> for OrderedSet<T>
where
    T: PartialEq,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let iter: _ = iter.into_iter();
        let size: usize = iter.size_hint().1.unwrap_or(0);

        let mut this: Self = Self::with_capacity(size);

        for item in iter {
            this.append(item);
        }

        this
    }
}

impl<T> TryFrom<Vec<T>> for OrderedSet<T>
where
    T: PartialEq,
{
    type Error = &'static str;

    fn try_from(other: Vec<T>) -> Result<Self, Self::Error> {
        let mut this: Self = Self::with_capacity(other.len());

        for item in other {
            if !this.append(item) {
                return Err("duplicate ordered set element");
            }
        }

        Ok(this)
    }
}
*/

#[derive(PartialEq, Eq, Debug, Default)]
pub struct OrderedSet<T>(pub Vec<T>);

impl<T: Ord> OrderedSet<T> {
    /// Create a new empty set
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Create a set from a `Vec`.
    /// `v` will be sorted and dedup first.
    pub fn from(mut v: Vec<T>) -> Self {
        v.sort();
        v.dedup();
        Self::from_sorted_set(v)
    }

    /// Create a set from a `Vec`.
    /// Assume `v` is sorted and contain unique elements.
    pub fn from_sorted_set(v: Vec<T>) -> Self {
        Self(v)
    }

    pub fn retain(&mut self, f: impl FnMut(&T) -> bool) {
        self.0.retain(f);
        self.0.sort_unstable();
    }

    /// Insert an element.
    /// Return true if insertion happened.
    pub fn insert(&mut self, value: T) -> bool {
        match self.0.binary_search(&value) {
            Ok(_) => false,
            Err(loc) => {
                self.0.insert(loc, value);
                true
            }
        }
    }

    /// Remove an element.
    /// Return true if removal happened.
    pub fn remove(&mut self, value: &T) -> bool {
        match self.0.binary_search(&value) {
            Ok(loc) => {
                self.0.remove(loc);
                true
            }
            Err(_) => false,
        }
    }

    /// Return if the set contains `value`
    pub fn contains(&self, value: &T) -> bool {
        self.0.binary_search(&value).is_ok()
    }

    /// Clear the set
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl<T: Ord> From<Vec<T>> for OrderedSet<T> {
    fn from(v: Vec<T>) -> Self {
        Self::from(v)
    }
}
impl<T> Deref for OrderedSet<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from() {
        let v = vec![4, 2, 3, 4, 3, 1];
        let set: OrderedSet<i32> = v.into();
        assert_eq!(set, OrderedSet::from(vec![1, 2, 3, 4]));
    }

    #[test]
    fn insert() {
        let mut set: OrderedSet<i32> = OrderedSet::new();
        assert_eq!(set, OrderedSet::from(vec![]));

        assert_eq!(set.insert(1), true);
        assert_eq!(set, OrderedSet::from(vec![1]));

        assert_eq!(set.insert(5), true);
        assert_eq!(set, OrderedSet::from(vec![1, 5]));

        assert_eq!(set.insert(3), true);
        assert_eq!(set, OrderedSet::from(vec![1, 3, 5]));

        assert_eq!(set.insert(3), false);
        assert_eq!(set, OrderedSet::from(vec![1, 3, 5]));
    }

    #[test]
    fn remove() {
        let mut set: OrderedSet<i32> = OrderedSet::from(vec![1, 2, 3, 4]);

        assert_eq!(set.remove(&5), false);
        assert_eq!(set, OrderedSet::from(vec![1, 2, 3, 4]));

        assert_eq!(set.remove(&1), true);
        assert_eq!(set, OrderedSet::from(vec![2, 3, 4]));

        assert_eq!(set.remove(&3), true);
        assert_eq!(set, OrderedSet::from(vec![2, 4]));

        assert_eq!(set.remove(&3), false);
        assert_eq!(set, OrderedSet::from(vec![2, 4]));

        assert_eq!(set.remove(&4), true);
        assert_eq!(set, OrderedSet::from(vec![2]));

        assert_eq!(set.remove(&2), true);
        assert_eq!(set, OrderedSet::from(vec![]));

        assert_eq!(set.remove(&2), false);
        assert_eq!(set, OrderedSet::from(vec![]));
    }

    #[test]
    fn contains() {
        let set: OrderedSet<i32> = OrderedSet::from(vec![1, 2, 3, 4]);

        assert_eq!(set.contains(&5), false);

        assert_eq!(set.contains(&1), true);

        assert_eq!(set.contains(&3), true);
    }

    #[test]
    fn clear() {
        let mut set: OrderedSet<i32> = OrderedSet::from(vec![1, 2, 3, 4]);
        set.clear();
        assert_eq!(set, OrderedSet::new());
    }
}
