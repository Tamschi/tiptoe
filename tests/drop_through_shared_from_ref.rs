#![cfg(feature = "sync")]

use tiptoe::{Arc, IntrusivelyCountable, TipToe};

#[derive(Default)]
struct Intruded {
	_nontrivial: Box<usize>,
	counter: TipToe,
}

unsafe impl IntrusivelyCountable for Intruded {
	type RefCounter = TipToe;

	fn ref_counter(&self) -> &Self::RefCounter {
		&self.counter
	}
}

#[test]
fn drop_through_shared_from_ref() {
	let a = Arc::pin(Intruded::default());
	let b = unsafe { Arc::borrow_pin_from_inner_ref(&&*a).clone() };
	drop(a);
	drop(b);
}
