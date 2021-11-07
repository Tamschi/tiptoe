//! Generic intrusive smart pointers for Rust.
//!
//! [![Zulip Chat](https://img.shields.io/endpoint?label=chat&url=https%3A%2F%2Fiteration-square-automation.schichler.dev%2F.netlify%2Ffunctions%2Fstream_subscribers_shield%3Fstream%3Dproject%252Ftiptoe)](https://iteration-square.schichler.dev/#narrow/stream/project.2Ftiptoe)
//!
//! > The library name is a pun:
//! >
//! > [`TipToe`] is a digit (counter) to be used as member that instances can "balance" on.
//! >
//! > The embedded counter is designed to be as unobtrusive as possible while not in use.
//!
//! # Features
//!
//! ## `"sync"`
//!
//! Enables the [`Arc`] type, which requires [`AtomicUsize`](`core::sync::atomic::AtomicUsize`).
//!
//! # Example
//!
//! ## Implementing [`TipToed`]
//!
//! ```rust
//! use pin_project::pin_project;
//! use tiptoe::{TipToe, TipToed};
//!
//! // All attributes optional.
//! #[pin_project]
//! #[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
//! pub struct A {
//!     #[pin]
//!     tip_toe: TipToe,
//! }
//!
//! unsafe impl TipToed for A {
//!     type RefCounter = TipToe;
//!
//!     fn tip_toe(&self) -> &TipToe {
//!         &self.tip_toe
//!     }
//! }
//! ```
//!
//! `A` is now ready for use with the intrusive pointers defined in this crate.
//!
//! The derived traits are optional, but included to show that [`TipToe`] doesn't interfere here (except for [`Copy`]).
//!
//! Using [pin-project](https://crates.io/crates/pin-project) is optional,
//! but very helpful if a struct should still be otherwise(!) mutable behind an intrusive pointer.
//!
//! Note that `A` must not be [`Unpin`] (in a way that would interfere with reference-counting).

#![doc(html_root_url = "https://docs.rs/tiptoe/0.0.1")]
#![warn(clippy::pedantic, missing_docs)]
#![allow(clippy::semicolon_if_nothing_returned)]
#![no_std]

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

extern crate alloc;

#[cfg(not(feature = "sync"))]
use core::cell::Cell;
#[cfg(feature = "sync")]
use core::sync::atomic::AtomicUsize;
use core::{
	cmp,
	hash::Hash,
	marker::PhantomPinned,
	mem::ManuallyDrop,
	ops::{Deref, DerefMut},
	pin::Pin,
};

#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
pub use sync::Arc;

/// Note: The `refcount` values [`EXCLUSIVITY_MARKER`] and up are special.
///
/// They denote an active exclusive borrow of the value, with some room to spare for data races.
const EXCLUSIVITY_MARKER: usize = usize::MAX - (usize::MAX - isize::MAX as usize) / 2;

/// An embeddable strong-only reference counter.
///
/// Transparent to [`PartialEq`], [`Eq`], [`PartialOrd`], [`Ord`] and [`Hash`], [clone](`Clone::clone`)d to its default.
///
/// Not [`Unpin`].
///
/// Always [`Send`] but [`Sync`] iff the `"sync"` feature is enabled.
#[derive(Debug, Default)]
pub struct TipToe {
	#[cfg(feature = "sync")]
	refcount: AtomicUsize,
	#[cfg(not(feature = "sync"))]
	refcount: Cell<usize>,
	_pinned: PhantomPinned,
}

impl TipToe {
	/// Creates as new [`TipToe`] instance.
	///
	/// > The name is a pun on this being a refcount digit (implementation detail: It's base [`usize::MAX`].) and
	/// > a member that the instance can stand on. If it "tips over" (becomes `0`) then the instance loses its
	/// > footing and may be dropped - or "caught" and moved elsewhere instead.
	#[must_use]
	pub fn new() -> Self {
		Self::default()
	}
}

impl Clone for TipToe {
	fn clone(&self) -> Self {
		Self::default()
	}
}

impl PartialEq for TipToe {
	fn eq(&self, _: &Self) -> bool {
		true
	}
}

impl Eq for TipToe {}

impl PartialOrd for TipToe {
	fn partial_cmp(&self, _: &Self) -> Option<cmp::Ordering> {
		Some(cmp::Ordering::Equal)
	}
}

impl Ord for TipToe {
	fn cmp(&self, _: &Self) -> cmp::Ordering {
		cmp::Ordering::Equal
	}
}

impl Hash for TipToe {
	fn hash<H: core::hash::Hasher>(&self, _: &mut H) {}
}

pub mod ref_counter_api {
	//! Low-level [`RefCounter`] API for custom intrusive reference-counting containers.

	use crate::{RefCounter, EXCLUSIVITY_MARKER};
	use abort::abort;

	#[cfg(any(feature = "sync", doc))]
	use core::sync::atomic::Ordering;

	mod private {
		#[cfg(not(feature = "sync"))]
		use core::cell::Cell;
		#[cfg(feature = "sync")]
		use core::sync::atomic::AtomicUsize;

		use crate::TipToe;

		pub trait Sealed: 'static {
			#[cfg(feature = "sync")]
			fn refcount(&self) -> &AtomicUsize;
			#[cfg(not(feature = "sync"))]
			fn refcount(&self) -> &Cell<usize>;

			fn refcount_ptr(&self) -> *mut usize {
				#[cfg(feature = "sync")]
				return self.refcount() as *const AtomicUsize as *mut usize;
				#[cfg(not(feature = "sync"))]
				return self.refcount().as_ptr();
			}
		}
		impl Sealed for TipToe {
			#[allow(clippy::inline_always)]
			#[cfg(feature = "sync")]
			#[inline(always)]
			fn refcount(&self) -> &AtomicUsize {
				&self.refcount
			}
			#[allow(clippy::inline_always)]
			#[cfg(not(feature = "sync"))]
			#[inline(always)]
			fn refcount(&self) -> &Cell<usize> {
				&self.refcount
			}
		}
	}
	pub(super) use private::Sealed;

	/// Common reference-count manipulation methods.
	pub trait RefCounterExt: RefCounter {
		/// Increments the reference count with [`Ordering::Relaxed`].
		///
		/// # Safety Notes
		///
		/// This is a safe operation, but incrementing the reference count too far will abort the current process rather than risk an overflow.
		///
		/// The (soft!) limit with the `"sync"` feature mirrors that of the standard library as of 2021-10-13.  
		/// The (soft!) limit without that feature will be somewhat higher.
		///
		/// # Panics
		///
		/// Iff called during exclusivity.
		///
		/// # Aborts
		///
		/// This function may abort in cases where the reference count becomes VERY high (for the given target platform),
		/// or during a race condition when dropping an [`Exclusivity`] erroneously while this function executes.
		fn increment(&self) {
			#[cfg(feature = "sync")]
			{
				let old_count = self.refcount().fetch_add(1, Ordering::Relaxed);
				if old_count >= (isize::MAX as usize) {
					if old_count >= EXCLUSIVITY_MARKER {
						// This is actually a handle clone during an exclusive borrow.
						// We'll revert the refcount and panic instead of aborting.
						// (TODO: Examine performance implications of having this branch here.)
						if self.refcount().fetch_sub(1, Ordering::Relaxed) > EXCLUSIVITY_MARKER {
							panic!("Tried to clone smart pointer during exclusive value borrow.")
						} else {
							// We likely got outraced by an `Exclusivity` drop.
							// That's quite badly erroneous and could cause data corruption elsewhere
							// due to the now most likely invalid reference count.
							abort()
						}
					} else {
						// See `alloc::Sync::Arc`'s clone implementation for why it's necessary to guard against immense reference counts:
						// <https://github.com/rust-lang/rust/blob/81117ff930fbf3792b4f9504e3c6bccc87b10823/library/alloc/src/sync.rs#L1327-L1338>
						//
						// In short:
						//
						// An overflow could cause a use-after free. There likely aren't about `isize::MAX` threads that can race here, though, and `isize::MAX` is a decently high limit.
						abort()
					}
				}
			}
			#[cfg(not(feature = "sync"))]
			{
				let old_count = self.refcount().get();
				if old_count >= EXCLUSIVITY_MARKER - 1 {
					if old_count < EXCLUSIVITY_MARKER {
						// See `alloc::rc::RcInnerPtr::inc_strong`:
						// <https://github.com/rust-lang/rust/blob/81117ff930fbf3792b4f9504e3c6bccc87b10823/library/alloc/src/rc.rs#L2442-L2453>
						abort()
					} else {
						// This is actually a handle clone during an exclusive borrow.
						// We'll panic instead of aborting. (TODO: Examine performance implications of having this branch here.)
						panic!("Tried to clone smart pointer during exclusive value borrow.")
					}
				}
				self.refcount().set(old_count + 1)
			};
		}

		/// Decrements the reference count with [`Ordering::Release`] and
		/// returns the **new** value.
		///
		/// # Safety
		///
		/// Must not be called during exclusivity.
		///
		/// In terms of memory-safety only:
		///
		/// Calling this method is equivalent to calling either [`Arc::from_raw`](`crate::Arc::from_raw`)
		/// or [`Rc::from_raw`](`crate::Rc::from_raw`) (whichever is safer)
		/// and then dropping the resulting instance.
		#[inline]
		unsafe fn decrement(&self) -> DecrementFollowup {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().fetch_sub(1, Ordering::Release)
				}
				#[cfg(not(feature = "sync"))]
				{
					let old_count = self.refcount().get();
					self.refcount().set(old_count - 1);
					old_count
				}
			} {
				1 => {
					#[cfg(feature = "sync")]
					self.refcount().load(Ordering::Acquire);
					DecrementFollowup::DropOrMoveIt
				}
				_ => DecrementFollowup::LeakIt,
			}
		}

		/// Decrements the reference count with [`Ordering::Relaxed`] and
		/// returns the **new** value.
		///
		/// # Safety Notes
		///
		/// This is only suitable for synchronous reference-counting.
		///
		/// # Safety
		///
		/// Must not be called during exclusivity.
		///
		/// In terms of memory-safety only:
		///
		/// Calling this method is equivalent to calling [`Rc::from_raw`](`crate::Rc::from_raw`)
		/// and then dropping the resulting instance.
		unsafe fn decrement_relaxed(&self) -> DecrementFollowup {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().fetch_sub(1, Ordering::Relaxed)
				}
				#[cfg(not(feature = "sync"))]
				{
					let old_count = self.refcount().get();
					self.refcount().set(old_count - 1);
					old_count
				}
			} {
				EXCLUSIVITY_MARKER..=usize::MAX => abort(),
				1 => DecrementFollowup::DropOrMoveIt,
				_ => DecrementFollowup::LeakIt,
			}
		}

		/// Checks for exclusivity with [`Ordering::Acquire`], and, if successful, prevents reference count increments until any resulting `Exclusivity` is dropped.
		///
		/// Returns [`None`] iff the reference-counted instance is shared.
		///
		/// # Safety
		///
		/// Dropping the [`Exclusivity`] performs a write to a remembered address, so **the borrowed instance must not be moved** until then.
		unsafe fn acquire(&self) -> Option<Exclusivity> {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().load(Ordering::Acquire)
				}
				#[cfg(not(feature = "sync"))]
				self.refcount().get()
			} {
				1 => Some(Exclusivity::new(self)),
				_ => None,
			}
		}

		/// Checks for exclusivity with [`Ordering::Relaxed`], and, if successful, prevents reference count increments until any resulting `Exclusivity` is dropped.
		///
		/// Returns [`None`] iff the reference-counted instance is shared.
		///
		/// # Safety
		///
		/// Exclusive references to the memory reference-counted by this instance may only exist while an [`Exclusivity`] does.
		/// (Forgetting it is fine but won't allow any further borrows of that memory at all.)
		///
		/// In particular, dropping the [`Exclusivity`] performs a write to a remembered address, so **the borrowed instance must not be moved**.
		///
		/// # Safety Notes
		///
		/// This is only suitable for synchronous reference-counting.
		#[must_use]
		fn acquire_relaxed(&self) -> Option<Exclusivity> {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().load(Ordering::Relaxed)
				}
				#[cfg(not(feature = "sync"))]
				self.refcount().get()
			} {
				1 => Some(Exclusivity::new(self)),
				_ => None,
			}
		}
	}
	impl<T> RefCounterExt for T where T: RefCounter {}

	/// An action to take after decrementing the reference-count.
	///
	/// This is a recommendation rather than a fixed requirement,
	/// but should be followed in most smart pointer and (other) container scenarios.
	pub enum DecrementFollowup {
		/// There are (usually!) further shared references.
		/// The instance should in most cases be left as-is.
		LeakIt,
		/// No further references are expected to exist now.
		/// The instance can (usually!) be dropped in place or, if [`Unpin`] or not pinned, moved now.
		DropOrMoveIt,
	}

	/// A handle for exclusively borrowing a reference-counted instance.
	///
	/// Any attempt to clone a handle will panic until this is dropped.
	pub struct Exclusivity {
		refcount: *mut usize,
		displaced_refcount: usize,
	}

	impl Exclusivity {
		fn new<T: ?Sized + Sealed>(counter: &T) -> Self {
			let ptr = counter.refcount_ptr();
			Self {
				displaced_refcount: unsafe { ptr.read() },
				refcount: {
					unsafe { ptr.write(EXCLUSIVITY_MARKER) };
					ptr
				},
			}
		}
	}

	impl Drop for Exclusivity {
		fn drop(&mut self) {
			unsafe { self.refcount.write(self.displaced_refcount) }
		}
	}
}
use ref_counter_api::{Exclusivity, Sealed};

/// `(Sealed)` Common trait of [`tiptoe`](`crate`)'s embeddable reference counter types.
pub trait RefCounter: Sealed {}
impl<T> RefCounter for T where T: Sealed {}

/// Enables intrusive reference counting for a structure.
///
/// # Safety
///
/// The returned [`TipToe`] must point to an instance embedded inside `Self` or semantically equivalent.
///
/// If the [`TipToe`] is embedded, then `Self` must be <code>**!**[Unpin]</code>!
///
/// > Hint: [`TipToe`] is `!Unpin`.
///
/// > The [`TipToe`] also mustn't be otherwise decremented (which can only be guaranteed if it's not public) in violation of sound reference-counting,
/// > but that's `unsafe` anyway.
///
/// [`TipToed::tip_toe`] must not have any effects, that is: It must not affect or effect any observable changes, other than through its return value.
///
/// > Mainly so the callee doesn't observe its address,
/// > which gives this crate a bit more flexibility regarding implementation details.
pub unsafe trait TipToed {
	/// [`TipToe`].
	type RefCounter: RefCounter;

	/// Gets a reference to the instance's reference counter.
	///
	/// > I highly recommend inlining this.
	fn tip_toe(&self) -> &TipToe;
}

unsafe impl<T> TipToed for ManuallyDrop<T>
where
	T: TipToed,
{
	type RefCounter = T::RefCounter;

	fn tip_toe(&self) -> &TipToe {
		#![allow(clippy::inline_always)]
		#![inline(always)]
		(**self).tip_toe()
	}
}

/// Exactly like [`Clone`] but with safety restrictions regarding usage.
///
/// See the methods for more information.
pub trait ManagedClone: Sized {
	/// # Safety
	///
	/// This method may only be used to create equally encapsulated instances.
	///
	/// For example, if you can see the instance is inside a [`Box`](`alloc::boxed::Box`),
	/// then you may clone it into another [`Box`](`alloc::boxed::Box`) this way.
	///
	/// If you have only a reference or pointer to the implementing type's instance,
	/// but don't know or can't replicate its precise encapsulation, then you must not call this method.
	///
	/// You may not use it in any way that could have side-effects before encapsulating the clone.
	/// This also means you may not drop the clone. Forgetting it is fine.
	unsafe fn managed_clone(&self) -> Self;

	/// # Safety
	///
	/// This method may only be used to create equally encapsulated instances.
	///
	/// For example, if you can see the instance is inside a [`Box`](`alloc::boxed::Box`),
	/// then you may clone it into another [`Box`](`alloc::boxed::Box`) this way.
	///
	/// If you have only a reference or pointer to the implementing type's instance,
	/// but don't know or can't replicate its precise encapsulation, then you must not call this method.
	///
	/// You may not use it in any way that could have side-effects before encapsulating the clone.
	/// This also means you may not drop the clone. Forgetting it is fine.
	unsafe fn managed_clone_from(&mut self, source: &Self) {
		*self = source.managed_clone()
	}
}

impl<T> ManagedClone for T
where
	T: Clone,
{
	unsafe fn managed_clone(&self) -> Self {
		self.clone()
	}

	unsafe fn managed_clone_from(&mut self, source: &Self) {
		self.clone_from(source)
	}
}

/// A [`Pin<&'a mut T>`](`Pin`), but also guarding against handle clones.
#[must_use]
pub struct ExclusivePin<'a, T: ?Sized> {
	reference: Pin<&'a mut T>,
	_exclusivity: Exclusivity,
}
impl<'a, T: ?Sized> ExclusivePin<'a, T> {
	/// Creates a new instance of [`ExclusivePin`] from a given [`Exclusivity`] and [`Pin<&mut T>`](`Pin`).
	pub fn new(exclusivity: Exclusivity, reference: Pin<&'a mut T>) -> Self {
		Self {
			reference,
			_exclusivity: exclusivity,
		}
	}
}

impl<'a, T: ?Sized> Deref for ExclusivePin<'a, T> {
	type Target = Pin<&'a mut T>;

	fn deref(&self) -> &Self::Target {
		&self.reference
	}
}

impl<'a, T: ?Sized> DerefMut for ExclusivePin<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.reference
	}
}
