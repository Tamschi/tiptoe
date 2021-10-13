use crate::{
	tip_toe_api::{AcquireOutcome, DecrementFollowup, TipToeExt},
	TipToed,
};
use alloc::{
	borrow::{Cow, ToOwned},
	boxed::Box,
};
use core::{
	any::{Any, TypeId},
	borrow::Borrow,
	fmt::{self, Debug, Display, Formatter, Pointer},
	hash::{Hash, Hasher},
	marker::PhantomData,
	mem::{self, ManuallyDrop},
	ops::Deref,
	pin::Pin,
	ptr::NonNull,
};
use tap::Pipe;

pub struct Arc<T: ?Sized + TipToed> {
	pointer: NonNull<T>,
	_phantom: PhantomData<T>,
}

impl<T: ?Sized + TipToed> AsRef<T> for Arc<T> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized + TipToed> Borrow<T> for Arc<T> {
	fn borrow(&self) -> &T {
		self
	}
}

impl<T: ?Sized + TipToed> Clone for Arc<T> {
	/// Makes a clone of this [`Arc`], pointing to the same instance.
	///
	/// This increases the strong reference count by 1.
	fn clone(&self) -> Self {
		self.tip_toe().increment();
		Self {
			pointer: self.pointer,
			_phantom: PhantomData,
		}
	}

	fn clone_from(&mut self, source: &Self) {
		if !Self::ptr_eq(self, source) {
			*self = source.clone()
		}
	}
}

impl<T: ?Sized + TipToed> Debug for Arc<T>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Arc").field(&&**self).finish()
	}
}

impl<T: ?Sized + TipToed> Default for Arc<T>
where
	T: Default,
{
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: ?Sized + TipToed> Deref for Arc<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.pointer.as_ref() }
	}
}

impl<T: ?Sized + TipToed> Display for Arc<T>
where
	T: Display,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		(**self).fmt(f)
	}
}

impl<T: ?Sized + TipToed> Drop for Arc<T> {
	fn drop(&mut self) {
		unsafe {
			match self.tip_toe().decrement() {
				DecrementFollowup::LeakIt => (),
				DecrementFollowup::DropOrMoveIt => drop(Box::from_raw(self.pointer.as_ptr())),
			}
		}
	}
}

impl<T: ?Sized + TipToed> Eq for Arc<T> where T: Eq {}

impl<T: ?Sized + TipToed> From<Box<T>> for Arc<T> {
	/// Converts a [`Box`] into an [`Arc`] without reallocating.
	fn from(box_: Box<T>) -> Self {
		box_.tip_toe().increment();
		unsafe { Self::from_raw(NonNull::new_unchecked(Box::leak(box_))) }
	}
}

impl<'a, B: ?Sized + TipToed> From<Cow<'a, B>> for Arc<B>
where
	B: ToOwned,
	Arc<B>: From<B::Owned>,
{
	/// Always converts into an exclusive instance,
	/// either by copying or by moving the value.
	fn from(cow: Cow<'a, B>) -> Self {
		match cow {
			Cow::Borrowed(b) => b.to_owned().into(),
			Cow::Owned(o) => o.into(),
		}
	}
}

impl<T: Sized + TipToed> From<T> for Arc<T> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: Sized + TipToed> From<T> for Pin<Arc<T>> {
	fn from(value: T) -> Self {
		Arc::pin(value)
	}
}

impl<T: ?Sized + TipToed> From<Pin<Arc<T>>> for Arc<T>
where
	T: Unpin,
{
	fn from(pinned: Pin<Arc<T>>) -> Self {
		Self::unpin(pinned)
	}
}

impl<T: ?Sized + TipToed> From<Arc<T>> for Pin<Arc<T>> {
	fn from(unpinned: Arc<T>) -> Self {
		unsafe { Pin::new_unchecked(unpinned) }
	}
}

impl<T: ?Sized + TipToed> Hash for Arc<T>
where
	T: Hash,
{
	fn hash<H: Hasher>(&self, state: &mut H) {
		(**self).hash(state)
	}
}

impl<T: ?Sized + TipToed> Ord for Arc<T>
where
	T: Ord,
{
	fn cmp(&self, other: &Self) -> core::cmp::Ordering {
		(**self).cmp(other)
	}
}

impl<T: ?Sized + TipToed, O: ?Sized + TipToed> PartialEq<Arc<O>> for Arc<T>
where
	T: PartialEq<O>,
{
	fn eq(&self, other: &Arc<O>) -> bool {
		(**self) == (**other)
	}
}

impl<T: ?Sized + TipToed, O: ?Sized + TipToed> PartialOrd<Arc<O>> for Arc<T>
where
	T: PartialOrd<O>,
{
	fn partial_cmp(&self, other: &Arc<O>) -> Option<core::cmp::Ordering> {
		(**self).partial_cmp(other)
	}
}

impl<T: ?Sized + TipToed> Pointer for Arc<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		Pointer::fmt(&self.pointer, f)
	}
}

unsafe impl<T: ?Sized + TipToed> Send for Arc<T> where T: Sync + Send {}
unsafe impl<T: ?Sized + TipToed> Sync for Arc<T> where T: Sync + Send {}
impl<T: ?Sized + TipToed> Unpin for Arc<T> {}

impl<T: ?Sized + TipToed> Arc<T> {
	#[must_use]
	pub fn new(value: T) -> Self
	where
		T: Sized,
	{
		value.tip_toe().increment();
		let instance = Box::leak(Box::new(value));
		unsafe { Self::from_raw(NonNull::new_unchecked(instance)) }
	}

	/// See also `impl From<Arc<_>> for Pin<Arc<_>>`.
	#[must_use]
	pub fn pin(value: T) -> Pin<Self>
	where
		T: Sized,
	{
		value.tip_toe().increment();
		let instance = Box::leak(Box::new(value));
		unsafe { Pin::new_unchecked(Self::from_raw(NonNull::new_unchecked(instance))) }
	}

	/// See also `impl From<Pin<Arc<_>>> for Arc<_>`.
	#[must_use]
	pub fn unpin(this: Pin<Self>) -> Self
	where
		T: Unpin,
	{
		unsafe { Pin::into_inner_unchecked(this) }
	}

	/// # Errors
	///
	/// Iff this [`Arc`] is not an exclusive handle.
	pub fn try_unwrap(this: Self) -> Result<T, Self>
	where
		T: Sized,
	{
		match this.tip_toe().acquire() {
			AcquireOutcome::Shared => Err(this),
			AcquireOutcome::Exclusive => unsafe {
				Ok(ManuallyDrop::take(
					&mut mem::transmute::<Self, Arc<ManuallyDrop<T>>>(this)
						.pointer
						.as_mut(),
				))
			},
		}
	}

	/// Constructs an [`Arc`] instance from a compatible value pointer.
	///
	/// # Safety
	///
	/// The pointer `raw_value` must have been created by leaking from a compatible *unpinned* container.
	///
	/// Containers are incompatible if their type parameter differs in a way that
	/// makes the equivalent pointer reinterpretation cast invalid.
	/// Otherwise:
	///
	/// ([`Arc`] and [`Rc`](`crate::Rc`) are compatible.
	/// [`Box`] is compatible iff the internal reference count had been incremented to at least `1` at the time of leaking.)
	///
	/// For every time the instance that pointer points to was leaked,
	/// this function must be called at most once.
	///
	/// The data `raw_value` points to may be in use only by [`Arc`].
	#[must_use = "Implicitly dropping this handle is likely a mistake."]
	pub unsafe fn from_raw(raw_value: NonNull<T>) -> Self {
		debug_assert_ne!(
			raw_value.as_ptr().cast::<()>() as usize,
			0,
			"Called `tiptoe::Arc::from_raw` with null pointer."
		);
		Self {
			pointer: raw_value,
			_phantom: PhantomData,
		}
	}

	/// Constructs a [pinned](`core::pin`) [`Arc`] instance from a compatible value pointer.
	///
	/// # Safety
	///
	/// The pointer `raw_value` must have been created by leaking from a compatible container.
	///
	/// Containers are incompatible if their type parameter differs in a way that
	/// makes the equivalent pointer reinterpretation cast invalid.
	/// Otherwise:
	///
	/// ([`Arc`] and [`Rc`](`crate::Rc`) are compatible.
	/// [`Box`] is compatible iff the internal reference count had been incremented to at least `1` at the time of leaking.)
	///
	/// For every time the instance that pointer points to was leaked,
	/// this function must be called at most once.
	///
	/// The data `raw_value` points to may be in use only by [`Arc`].
	#[must_use = "Implicitly dropping this handle is likely a mistake."]
	pub unsafe fn pinned_from_raw(raw_value: NonNull<T>) -> Pin<Self> {
		debug_assert_ne!(
			raw_value.as_ptr().cast::<()>() as usize,
			0,
			"Called `tiptoe::Arc::from_raw` with null pointer."
		);
		Self {
			pointer: raw_value,
			_phantom: PhantomData,
		}
		.into()
	}

	#[must_use]
	pub fn leak(this: Self) -> NonNull<T> {
		let pointer = this.pointer;
		mem::forget(this);
		pointer
	}

	/// # Safety Notes
	///
	/// Keep in mind that the pinning invariants, including the drop guarantee, must still be upheld.
	#[must_use]
	pub fn leak_pinned(this: Pin<Self>) -> NonNull<T> {
		let this = unsafe { Pin::into_inner_unchecked(this) };
		let pointer = this.pointer;
		mem::forget(this);
		pointer
	}

	#[must_use]
	pub fn ptr_eq(this: &Self, other: &Self) -> bool {
		this.pointer == other.pointer
	}

	#[must_use]
	pub fn make_mut(this: &mut Pin<Self>) -> Pin<&mut T>
	where
		T: Sized + Clone,
	{
		match this.tip_toe().acquire() {
			AcquireOutcome::Exclusive => (),
			AcquireOutcome::Shared => *this = (&**this).clone().pipe(Self::pin),
		}
		unsafe {
			Pin::new_unchecked(
				mem::transmute_copy::<Pin<Self>, Self>(this)
					.pointer
					.as_mut(),
			)
		}
	}

	#[must_use]
	pub fn get_mut(this: &mut Pin<Self>) -> Option<Pin<&mut T>> {
		match this.tip_toe().acquire() {
			AcquireOutcome::Shared => None,
			AcquireOutcome::Exclusive => Some(unsafe {
				Pin::new_unchecked(
					mem::transmute_copy::<Pin<Self>, Self>(this)
						.pointer
						.as_mut(),
				)
			}),
		}
	}

	/// Attempts to cast this [`Arc`] into once of concrete type `U`.
	///
	/// # Errors
	///
	/// Iff the underlying instance isn't a `U`.
	pub fn downcast<U>(this: Self) -> Result<Arc<U>, Self>
	where
		T: Any,
		U: Any + TipToed,
	{
		if Any::type_id(&*this) == TypeId::of::<U>() {
			Ok(unsafe { Arc::from_raw(Arc::leak(this).cast()) })
		} else {
			Err(this)
		}
	}

	/// Attempts to cast this [`Arc`] into once of concrete type `U`.
	///
	/// # Errors
	///
	/// Iff the underlying instance isn't a `U`.
	pub fn downcast_pinned<U>(this: Pin<Self>) -> Result<Pin<Arc<U>>, Pin<Self>>
	where
		T: Any,
		U: Any + TipToed,
	{
		if Any::type_id(&*this) == TypeId::of::<U>() {
			Ok(unsafe { Arc::pinned_from_raw(Arc::leak_pinned(this).cast()) })
		} else {
			Err(this)
		}
	}
}
