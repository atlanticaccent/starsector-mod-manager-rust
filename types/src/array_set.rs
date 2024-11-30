use std::{fmt::Debug, mem::MaybeUninit};

use typenum::{Const, IsLessOrEqual, NonZero, ToUInt, U};

pub struct ArraySet<T, const CAP: usize> {
  mem: [MaybeUninit<T>; CAP],
  len: usize,
}

impl<T> ArraySet<T, 0> {
  pub const fn new<const CAP: usize>() -> ArraySet<T, CAP> {
    ArraySet {
      mem: uninit_array(),
      len: 0,
    }
  }
}

impl<T: PartialEq, const CAP: usize> ArraySet<T, CAP> {
  pub fn insert(&mut self, val: T) -> bool {
    if self.len == CAP || self.contains(&val) {
      return false;
    }

    self.mem[self.len] = MaybeUninit::new(val);
    self.len += 1;

    true
  }

  pub fn remove(&mut self, val: &T) -> bool {
    if let Some(idx) = self.index(val) {
      let last = self.len - 1;
      self.as_mut().swap(idx, last);
      unsafe { MaybeUninit::assume_init_drop(&mut self.mem[last]) };
      self.len -= 1;

      true
    } else {
      false
    }
  }

  pub fn contains(&self, val: &T) -> bool {
    self.as_ref().contains(&val)
  }

  pub fn index(&self, val: &T) -> Option<usize> {
    for (idx, elem) in self.as_ref().iter().enumerate() {
      if elem == val {
        return Some(idx);
      }
    }

    None
  }

  pub fn clear(&mut self) {
    if !self.is_empty() {
      self.mem = uninit_array();
      self.len = 0;
    }
  }

  #[inline(always)]
  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  #[inline(always)]
  pub fn len(&self) -> usize {
    self.len
  }
}

#[inline(always)]
const fn uninit_array<T, const CAP: usize>() -> [MaybeUninit<T>; CAP] {
  [const { MaybeUninit::uninit() }; CAP]
}

impl<T: PartialEq, const CAP: usize> PartialEq for ArraySet<T, CAP> {
  fn eq(&self, other: &Self) -> bool {
    self.len == other.len
      && self
        .as_ref()
        .iter()
        .zip(other.as_ref())
        .all(|(left, right)| left == right)
  }
}

impl<T, const CAP: usize> Default for ArraySet<T, CAP> {
  fn default() -> Self {
    ArraySet::new()
  }
}

impl<T: Debug, const CAP: usize> Debug for ArraySet<T, CAP> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ArraySet")
      .field("mem", &self.as_ref())
      .field("len", &self.len)
      .finish()
  }
}

impl<T: Clone, const CAP: usize> Clone for ArraySet<T, CAP> {
  fn clone(&self) -> Self {
    let mut new = Self::default();
    new.len = self.len;
    new.as_mut().clone_from_slice(self.as_ref());

    new
  }
}

impl<T, const CAP: usize> AsRef<[T]> for ArraySet<T, CAP> {
  #[inline(always)]
  fn as_ref(&self) -> &[T] {
    unsafe { MaybeUninit::slice_assume_init_ref(&self.mem[0..self.len]) }
  }
}

impl<T, const CAP: usize> AsMut<[T]> for ArraySet<T, CAP> {
  #[inline(always)]
  fn as_mut(&mut self) -> &mut [T] {
    unsafe { MaybeUninit::slice_assume_init_mut(&mut self.mem[0..self.len]) }
  }
}

#[non_exhaustive]
#[derive(Debug)]
pub enum ArraySetError {
  ContainsDuplicates,
  LengthMismatch { max: usize, provided: usize },
}

impl<T: PartialEq, const LEN: usize, const CAP: usize> TryFrom<[T; LEN]> for ArraySet<T, CAP>
where
  Const<LEN>: ToUInt,
  Const<CAP>: ToUInt,
  U<LEN>: IsLessOrEqual<U<CAP>>,
  <U<LEN> as IsLessOrEqual<U<CAP>>>::Output: NonZero,
{
  type Error = ArraySetError;

  fn try_from(value: [T; LEN]) -> Result<Self, Self::Error> {
    let mut new = Self::default();
    for (idx, elem) in value.into_iter().enumerate() {
      for prev in 0..idx {
        if unsafe { new.mem[prev].assume_init_ref() } == &elem {
          return Err(ArraySetError::ContainsDuplicates);
        }
      }
      new.mem[idx] = MaybeUninit::new(elem);
    }

    Ok(new)
  }
}

impl<T: Clone + PartialEq, const CAP: usize> TryFrom<&[T]> for ArraySet<T, CAP> {
  type Error = ArraySetError;

  fn try_from(value: &[T]) -> Result<Self, Self::Error> {
    if value.len() > CAP {
      return Err(ArraySetError::LengthMismatch {
        max: CAP,
        provided: value.len(),
      });
    }

    let mut new = Self::default();
    for (idx, elem) in value.iter().enumerate() {
      for prev in 0..idx {
        if unsafe { new.mem[prev].assume_init_ref() } == elem {
          return Err(ArraySetError::ContainsDuplicates);
        }
      }
      new.mem[idx] = MaybeUninit::new(elem.clone());
    }

    Ok(new)
  }
}

impl<T, const CAP: usize> Drop for ArraySet<T, CAP> {
  fn drop(&mut self) {
    unsafe {
      for elem in &mut self.mem[0..self.len] {
        MaybeUninit::assume_init_drop(elem);
      }
    }
  }
}

#[cfg(test)]
mod test {
  use std::{
    hash::{BuildHasher, Hasher, RandomState},
    rc::{Rc, Weak},
  };

  use crate::ArraySet;

  struct DropTracker {
    live: Vec<Weak<u64>>,
  }

  impl DropTracker {
    pub fn new() -> Self {
      Self { live: Vec::new() }
    }

    pub fn build(&mut self) -> DropCheck {
      let random = RandomState::new().build_hasher().finish();
      let marker = Rc::new(random);

      self.live.push(Rc::downgrade(&marker));
      let drop_chk = DropCheck(marker);

      drop_chk
    }

    pub fn all_live(&self) -> bool {
      for weak in &self.live {
        if weak.strong_count() == 0 {
          return false;
        }
      }

      true
    }

    pub fn all_dead(&self) -> bool {
      for weak in &self.live {
        if weak.strong_count() > 0 {
          return false;
        }
      }

      true
    }
  }

  #[derive(PartialEq, Eq, Hash, Debug)]
  struct DropCheck(Rc<u64>);

  #[test]
  fn basic() {
    let mut set: ArraySet<usize, 10> = ArraySet::new();

    // Can insert 0 to =9
    for elem in 0..10 {
      assert!(set.insert(elem));
      // Set size reported correctly
      assert_eq!(set.len(), elem + 1);
      // Cannot insert item that is already in set
      assert!(!set.insert(elem));
      // Set size reported correctly
      assert_eq!(set.len(), elem + 1);

      // Set contains all previously inserted items
      for prev in 0..=elem {
        assert!(set.contains(&prev))
      }
    }
    // Set size is 10 after inserting 10 items
    assert_eq!(set.len(), 10);

    // Cannot insert into full set
    assert!(!set.insert(10));
    // Set size is still 10
    assert_eq!(set.len(), 10);

    // Can remove inserted items
    for elem in 0..10 {
      assert!(set.remove(&elem));
      // Set size reported correctly
      assert_eq!(set.len(), 9 - elem)
    }
    // Set size is 0 after removing all items
    assert_eq!(set.len(), 0);

    // (Set does not contain/cannot remove) previously-inserted-but-removed items
    for elem in 0..10 {
      assert!(!set.contains(&elem));
      assert!(!set.remove(&elem));
      // Set size is still 0
      assert_eq!(set.len(), 0)
    }
    // Set size is still 0
    assert_eq!(set.len(), 0);

    // Can convert from array
    let array: [usize; 10] = std::array::from_fn(|i| i);
    set = ArraySet::try_from(array).expect("Convert");
    set.clear();

    // Can convert from slice
    let array: [usize; 10] = std::array::from_fn(|i| i);
    set = ArraySet::try_from(array.as_slice()).expect("Convert");

    // Set does not contain items that were never inserted
    for elem in 10..20 {
      assert!(!set.contains(&elem))
    }

    // Can clear set
    set.clear();
    assert!(set.is_empty());
    assert_eq!(set.len(), 0);

    // Can reuse set after clearing it
    for elem in 10..20 {
      assert!(set.insert(elem));
      // Set size reported correctly
      assert_eq!(set.len(), elem - 9);
      // Cannot insert item that is already in set
      assert!(!set.insert(elem));
      // Set size reported correctly
      assert_eq!(set.len(), elem - 9);

      // Set contains all previously inserted items
      for prev in 10..=elem {
        assert!(set.contains(&prev))
      }
    }

    set.clear();
    // Check set is cloneable
    for elem in 0..10 {
      assert!(set.insert(elem));

      let other = set.clone();
      assert_eq!(set, other)
    }

    let mut set: ArraySet<DropCheck, 10> = ArraySet::new();
    let mut tracker = DropTracker::new();

    (0..10).for_each(|_| assert!(set.insert(tracker.build())));
    assert!(tracker.all_live());

    drop(set);
    assert!(tracker.all_dead());
  }
}
