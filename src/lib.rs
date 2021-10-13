#![doc(html_root_url = "https://docs.rs/tiptoe/0.0.1")]
#![warn(clippy::pedantic)]
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
use core::{cmp, hash::Hash, marker::PhantomPinned, mem::ManuallyDrop};

#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
pub use sync::Arc;

/// A member that an instance can balance on.
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

pub mod tip_toe_api {
	//! Low-level [`TipToe`] API for custom intrusive reference-counting containers.

	#[cfg(feature = "sync")]
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

	use crate::TipToe;

	pub trait TipToeExt: Sealed {
		/// Increments the reference count with [`Ordering::Relaxed`].
		fn increment(&self) {
			#[cfg(feature = "sync")]
			self.refcount().fetch_add(1, Ordering::Relaxed);
			#[cfg(not(feature = "sync"))]
			self.refcount().set(self.refcount().get() + 1)
		}

		/// Decrements the reference count with [`Ordering::Release`] and
		/// returns the **new** value.
		///
		/// # Safety
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
		/// In terms of memory-safety only:
		///
		/// Calling this method is equivalent to calling either [`Rc::from_raw`](`crate::Rc::from_raw`)
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
				1 => DecrementFollowup::DropOrMoveIt,
				_ => DecrementFollowup::LeakIt,
			}
		}

		/// Loads the reference count with [`Ordering::Acquire`].
		fn acquire(&self) -> AcquireOutcome {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().load(Ordering::Acquire)
				}
				#[cfg(not(feature = "sync"))]
				self.refcount().get()
			} {
				1 => AcquireOutcome::Exclusive,
				_ => AcquireOutcome::Shared,
			}
		}

		/// Loads the reference count with [`Ordering::Relaxed`]
		///
		/// # Safety Notes
		///
		/// This is only suitable for synchronous reference-counting.
		fn acquire_relaxed(&self) -> AcquireOutcome {
			match {
				#[cfg(feature = "sync")]
				{
					self.refcount().load(Ordering::Relaxed)
				}
				#[cfg(not(feature = "sync"))]
				self.refcount().get()
			} {
				1 => AcquireOutcome::Exclusive,
				_ => AcquireOutcome::Shared,
			}
		}
	}
	impl TipToeExt for TipToe {}

	pub enum DecrementFollowup {
		LeakIt,
		DropOrMoveIt,
	}

	pub enum AcquireOutcome {
		Exclusive,
		Shared,
	}
}
use tip_toe_api::Sealed;

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
pub unsafe trait TipToed {
	/// [`TipToe`].
	type Toe: Sealed;

	/// > I recommend inlining this.
	#[allow(unused_attributes)]
	fn tip_toe(&self) -> &TipToe {
		#![inline(always)]
		todo!() // Filled in by implementor.
	}
}

unsafe impl<T> TipToed for ManuallyDrop<T>
where
	T: TipToed,
{
	type Toe = T::Toe;

	fn tip_toe(&self) -> &TipToe {
		#![allow(clippy::inline_always)]
		#![inline(always)]
		(**self).tip_toe()
	}
}
