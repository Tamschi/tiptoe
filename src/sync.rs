use crate::{
	ref_counter_api::{DecrementFollowup, RefCounterExt},
	ExclusivePin, IntrusivelyCountable, ManagedClone,
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
	mem::{self, ManuallyDrop},
	ops::Deref,
	pin::Pin,
	ptr::NonNull,
};
use tap::{Pipe, Tap};

// spell-checker:ignore eference ounted
/// An **a**synchronously **r**eference-**c**ounted smart pointer (copy-on-write single-item container).
///
/// Unlike with [`alloc::sync::Arc`], the reference-count must be embedded in the payload instance itself.
#[repr(transparent)]
pub struct Arc<T: ?Sized + IntrusivelyCountable> {
	pointer: NonNull<T>,
}

impl<T: ?Sized + IntrusivelyCountable> AsRef<T> for Arc<T> {
	fn as_ref(&self) -> &T {
		self
	}
}

impl<T: ?Sized + IntrusivelyCountable> Borrow<T> for Arc<T> {
	fn borrow(&self) -> &T {
		self
	}
}

impl<T: ?Sized + IntrusivelyCountable> Clone for Arc<T> {
	/// Makes a clone of this [`Arc`], pointing to the same instance.
	///
	/// This increases the strong reference count by 1.
	fn clone(&self) -> Self {
		self.ref_counter().increment();
		Self {
			pointer: self.pointer,
		}
	}

	fn clone_from(&mut self, source: &Self) {
		if !Self::ptr_eq(self, source) {
			*self = source.clone()
		}
	}
}

impl<T: ?Sized + IntrusivelyCountable> Debug for Arc<T>
where
	T: Debug,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		f.debug_tuple("Arc").field(&&**self).finish()
	}
}

impl<T: ?Sized + IntrusivelyCountable> Default for Arc<T>
where
	T: Default,
{
	fn default() -> Self {
		Self::new(T::default())
	}
}

impl<T: ?Sized + IntrusivelyCountable> Deref for Arc<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.pointer.as_ref() }
	}
}

impl<T: ?Sized + IntrusivelyCountable> Display for Arc<T>
where
	T: Display,
{
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		(**self).fmt(f)
	}
}

impl<T: ?Sized + IntrusivelyCountable> Drop for Arc<T> {
	fn drop(&mut self) {
		unsafe {
			match self.ref_counter().decrement() {
				DecrementFollowup::LeakIt => (),
				DecrementFollowup::DropOrMoveIt => drop(Box::from_raw(self.pointer.as_ptr())),
			}
		}
	}
}

impl<T: ?Sized + IntrusivelyCountable> Eq for Arc<T> where T: Eq {}

impl<T: ?Sized + IntrusivelyCountable> From<Box<T>> for Arc<T> {
	/// Converts a [`Box`] into an [`Arc`] without reallocating.
	fn from(box_: Box<T>) -> Self {
		box_.ref_counter().increment();
		unsafe { Self::from_raw(NonNull::new_unchecked(Box::leak(box_))) }
	}
}

impl<'a, B: ?Sized + IntrusivelyCountable> From<Cow<'a, B>> for Arc<B>
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

impl<T: Sized + IntrusivelyCountable> From<T> for Arc<T> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}

impl<T: Sized + IntrusivelyCountable> From<T> for Pin<Arc<T>> {
	fn from(value: T) -> Self {
		Arc::pin(value)
	}
}

impl<T: ?Sized + IntrusivelyCountable> From<Pin<Arc<T>>> for Arc<T>
where
	T: Unpin,
{
	fn from(pinned: Pin<Arc<T>>) -> Self {
		unsafe { Pin::into_inner_unchecked(pinned) }
	}
}

impl<T: ?Sized + IntrusivelyCountable> From<Arc<T>> for Pin<Arc<T>>
where
	T: Unpin,
{
	fn from(unpinned: Arc<T>) -> Self {
		unsafe { Pin::new_unchecked(unpinned) }
	}
}

impl<T: ?Sized + IntrusivelyCountable> Hash for Arc<T>
where
	T: Hash,
{
	fn hash<H: Hasher>(&self, state: &mut H) {
		(**self).hash(state)
	}
}

impl<T: ?Sized + IntrusivelyCountable> Ord for Arc<T>
where
	T: Ord,
{
	fn cmp(&self, other: &Self) -> core::cmp::Ordering {
		(**self).cmp(other)
	}
}

impl<T: ?Sized + IntrusivelyCountable, O: ?Sized + IntrusivelyCountable> PartialEq<Arc<O>>
	for Arc<T>
where
	T: PartialEq<O>,
{
	fn eq(&self, other: &Arc<O>) -> bool {
		(**self) == (**other)
	}
}

impl<T: ?Sized + IntrusivelyCountable, O: ?Sized + IntrusivelyCountable> PartialOrd<Arc<O>>
	for Arc<T>
where
	T: PartialOrd<O>,
{
	fn partial_cmp(&self, other: &Arc<O>) -> Option<core::cmp::Ordering> {
		(**self).partial_cmp(other)
	}
}

impl<T: ?Sized + IntrusivelyCountable> Pointer for Arc<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		Pointer::fmt(&self.pointer, f)
	}
}

unsafe impl<T: ?Sized + IntrusivelyCountable> Send for Arc<T> where T: Sync + Send {}
unsafe impl<T: ?Sized + IntrusivelyCountable> Sync for Arc<T> where T: Sync + Send {}
impl<T: ?Sized + IntrusivelyCountable> Unpin for Arc<T> {}

impl<T: ?Sized + IntrusivelyCountable> Arc<T> {
	/// Creates a new instance of [`Arc<_>`] by moving `value` into a new heap allocation.
	///
	/// This increases the intrusive reference-count by 1.
	///
	/// Calling this method with an instance with non-zero reference-count is safe,
	/// but likely to lead to memory leaks (or the process being aborted, if the recorded count is very high).
	#[must_use]
	pub fn new(value: T) -> Self
	where
		T: Sized,
	{
		value.ref_counter().increment();
		let instance = Box::leak(Box::new(value));
		unsafe { Self::from_raw(NonNull::new_unchecked(instance)) }
	}

	/// Creates a new instance of [`Pin<Arc<_>>`](`Arc`) by moving `value` into a new heap allocation.
	///
	/// This increases the intrusive reference-count by 1.
	///
	/// Calling this method with an instance with non-zero reference-count is safe,
	/// but likely to lead to memory leaks (or the process being aborted, if the recorded count is very high).
	#[must_use]
	pub fn pin(value: T) -> Pin<Self>
	where
		T: Sized,
	{
		value.ref_counter().increment();
		let instance = Box::leak(Box::new(value));
		unsafe { Pin::new_unchecked(Self::from_raw(NonNull::new_unchecked(instance))) }
	}

	/// # Errors
	///
	/// Iff this [`Arc`] is not an exclusive handle.
	pub fn try_unpin(this: Pin<Self>) -> Result<T, Pin<Self>>
	where
		T: Sized + Unpin,
	{
		Pin::into_inner(this)
			.pipe(Self::try_unwrap)
			.map_err(Pin::new)
	}

	/// # Errors
	///
	/// Iff this [`Arc`] is not an exclusive handle.
	pub fn try_unwrap(this: Self) -> Result<T, Self>
	where
		T: Sized,
	{
		match unsafe { this.ref_counter().acquire() } {
			None => Err(this),
			Some(exclusivity) => unsafe {
				drop(exclusivity); // We still have exclusivity until we relinquish control. However, we do want to manipulate the reference count.
				Ok(ManuallyDrop::take(
					&mut mem::transmute::<Self, Arc<ManuallyDrop<T>>>(this)
						.pointer
						.as_mut(),
				)
				.tap_mut(|unwrapped| unwrapped.ref_counter().decrement_relaxed().pipe(drop)))
			},
		}
	}

	/// Constructs an [`Arc`] instance from a compatible value pointer.
	///
	/// # Safety
	///
	/// The pointer `raw_value` must have been created by leaking the heap-allocated value in a compatible *unpinned* container.
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
		Self { pointer: raw_value }
	}

	/// Constructs a [pinned](`core::pin`) [`Arc`] instance from a compatible value pointer.
	///
	/// # Safety
	///
	/// The pointer `raw_value` must have been created by leaking the heap-allocated value in a compatible container.
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
		Self { pointer: raw_value }.pipe(|this| Pin::new_unchecked(this))
	}

	/// Unsafely borrows a shared reference to an [`Arc`]-managed instance as [`Arc`].
	///
	/// This is purely a reinterpretation cast.
	///
	/// # Safety
	///
	/// `inner` must be a reference to a reference to an instance managed by [`Arc`].
	#[must_use]
	pub unsafe fn borrow_from_inner_ref<'a>(inner: &'a &'a T) -> &'a Self {
		&*(inner as *const &T).cast::<Self>()
	}

	/// Unsafely borrows a shared reference to a [`Pin<Arc>`]-managed instance as [`Pin<Arc>`].
	///
	/// This is purely a reinterpretation cast.
	///
	/// # Safety
	///
	/// `inner` must be a reference to a reference to an instance managed by [`Pin<Arc>`].
	#[must_use]
	pub unsafe fn borrow_pin_from_inner_ref<'a>(inner: &'a &'a T) -> &'a Pin<Self> {
		&*(inner as *const &T).cast::<Pin<Self>>()
	}

	/// Unwraps the payload pointer contained in the current instance.
	///
	/// This does not decrease the reference-count.
	#[must_use = "Ignoring this pointer will usually lead to the underlying payload instance leaking."]
	pub fn leak(this: Self) -> NonNull<T> {
		let pointer = this.pointer;
		mem::forget(this);
		pointer
	}

	/// Unwraps the payload pointer contained in the current instance.
	///
	/// This does not decrease the reference-count.
	///
	/// # Safety Notes
	///
	/// Keep in mind that the pinning invariants, including the drop guarantee, must still be upheld.
	#[must_use = "Ignoring this pointer will usually lead to the underlying payload instance leaking."]
	pub fn leak_pinned(this: Pin<Self>) -> NonNull<T> {
		let this = unsafe { Pin::into_inner_unchecked(this) };
		let pointer = this.pointer;
		mem::forget(this);
		pointer
	}

	/// Checks whether two instances of [`Arc<T>`] point to the same instance.
	#[must_use]
	pub fn ptr_eq(this: &Self, other: &Self) -> bool {
		this.pointer == other.pointer
	}

	/// Ensures the payload is exclusively pointed to by this [`Arc<T>`], cloning it if necessary,
	/// and gives access to a [`Pin<&mut T>`] that safely can *not* be used to clone the [`Arc<T>`].
	pub fn make_mut(this: &mut Pin<Self>) -> ExclusivePin<T>
	where
		T: Sized + ManagedClone,
	{
		let exclusivity = unsafe { this.ref_counter().acquire() }.unwrap_or_else(|| {
			*this = unsafe {
				// Safety:
				// No effective encapsulation change happens.
				// `Self::pin` does call `IntrusivelyCountable::ref_counter`, but this is legal as that method is not allowed to have effects.
				(&**this).managed_clone().pipe(Self::pin)
			};

			// This could be done faster, but whether that's significant is up to benchmarking it.
			unsafe { this.ref_counter().acquire() }.expect("unreachable")
		});

		ExclusivePin::new(exclusivity, unsafe {
			Pin::new_unchecked(
				(*(this as *mut Pin<Self>).cast::<Arc<T>>())
					.pointer
					.as_mut(),
			)
		})
	}

	/// Checks whether the payload is exclusively pointed to by this [`Arc<T>`] and, if this is the case,
	/// gives access to a [`Pin<&mut T>`] that safely can *not* be used to clone the [`Arc<T>`].
	#[must_use]
	pub fn get_mut(this: &mut Pin<Self>) -> Option<ExclusivePin<T>> {
		unsafe { this.ref_counter().acquire() }.map(|exclusivity| {
			ExclusivePin::new(exclusivity, unsafe {
				Pin::new_unchecked(
					(*(this as *mut Pin<Self>).cast::<Arc<T>>())
						.pointer
						.as_mut(),
				)
			})
		})
	}

	/// Attempts to cast this [`Arc`] into once of concrete type `U`.
	///
	/// # Errors
	///
	/// Iff the underlying instance isn't a `U`.
	pub fn downcast<U>(this: Self) -> Result<Arc<U>, Self>
	where
		T: Any,
		U: Any + IntrusivelyCountable,
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
		U: Any + IntrusivelyCountable,
	{
		if Any::type_id(&*this) == TypeId::of::<U>() {
			Ok(unsafe { Arc::pinned_from_raw(Arc::leak_pinned(this).cast()) })
		} else {
			Err(this)
		}
	}
}
