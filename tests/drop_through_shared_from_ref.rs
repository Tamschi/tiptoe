#![cfg(feature = "sync")]

use std::cell::UnsafeCell;

use tiptoe::{Arc, IntrusivelyCountable, TipToe};

#[derive(Default)]
struct Intruded {
	intruded: UnsafeCell<Intruded_>,
}

#[derive(Default)]
struct Intruded_ {
	_nontrivial: Box<usize>,
	counter: TipToe,
}

unsafe impl IntrusivelyCountable for Intruded {
	type RefCounter = TipToe;

	fn ref_counter(&self) -> &Self::RefCounter {
		&unsafe { &*self.intruded.get() }.counter
	}
}

#[test]
fn drop_through_shared_from_ref() {
	let a = Arc::pin(Intruded::default());
	let b = unsafe { Arc::borrow_pin_from_inner_ref(&&*a).clone() };
	drop(a);
	drop(b);
}
