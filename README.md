# tiptoe

[![Lib.rs](https://img.shields.io/badge/Lib.rs-*-84f)](https://lib.rs/crates/tiptoe)
[![Crates.io](https://img.shields.io/crates/v/tiptoe)](https://crates.io/crates/tiptoe)
[![Docs.rs](https://docs.rs/tiptoe/badge.svg)](https://docs.rs/tiptoe)

![Rust 1.54](https://img.shields.io/static/v1?logo=Rust&label=&message=1.54&color=grey)
[![CI](https://github.com/Tamschi/tiptoe/workflows/CI/badge.svg?branch=develop)](https://github.com/Tamschi/tiptoe/actions?query=workflow%3ACI+branch%3Adevelop)
![Crates.io - License](https://img.shields.io/crates/l/tiptoe/0.0.2)

[![GitHub](https://img.shields.io/static/v1?logo=GitHub&label=&message=%20&color=grey)](https://github.com/Tamschi/tiptoe)
[![open issues](https://img.shields.io/github/issues-raw/Tamschi/tiptoe)](https://github.com/Tamschi/tiptoe/issues)
[![open pull requests](https://img.shields.io/github/issues-pr-raw/Tamschi/tiptoe)](https://github.com/Tamschi/tiptoe/pulls)
[![good first issues](https://img.shields.io/github/issues-raw/Tamschi/tiptoe/good%20first%20issue?label=good+first+issues)](https://github.com/Tamschi/tiptoe/contribute)

[![crev reviews](https://web.crev.dev/rust-reviews/badge/crev_count/tiptoe.svg)](https://web.crev.dev/rust-reviews/crate/tiptoe/)
[![Zulip Chat](https://img.shields.io/endpoint?label=chat&url=https%3A%2F%2Fiteration-square-automation.schichler.dev%2F.netlify%2Ffunctions%2Fstream_subscribers_shield%3Fstream%3Dproject%252Ftiptoe)](https://iteration-square.schichler.dev/#narrow/stream/project.2Ftiptoe)

An easy-to-support intrusively reference-counting smart pointer.

<!-- TODO: \rs. Implement once non-generically for both `Arc` and `Rc`, no overhead. -->

## Installation

Please use [cargo-edit](https://crates.io/crates/cargo-edit) to always add the latest version of this library:

```cmd
cargo add tiptoe --features sync
```

## Features

### `"sync"`

Enables the [`Arc`](https://docs.rs/tiptoe/latest/tiptoe/struct.Arc.html) type, which requires [`AtomicUsize`](https://doc.rust-lang.org/stable/core/sync/atomic/struct.AtomicUsize.html).

## Example

```rust
use pin_project::pin_project;
use tiptoe::{IntrusivelyCountable, TipToe};

// All attributes optional.
#[pin_project]
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct A {
    #[pin]
    ref_counter: TipToe,
}

unsafe impl IntrusivelyCountable for A {
    type RefCounter = TipToe;

    fn ref_counter(&self) -> &Self::RefCounter {
        &self.ref_counter
    }
}

fn main() {
    #[cfg(feature = "sync")]
    {
        use tiptoe::Arc;

        let arc = Arc::new(A::default());
        let inner_ref: &A = &arc;

        let another_arc = unsafe {
            Arc::borrow_from_inner_ref(&inner_ref)
        }.clone();
    }
}
```

## Implementation Progress

This library is currently only implemented about as far as I need it for rhizome.

Please have a look at the [issues](https://github.com/Tamschi/tiptoe/issues) if you'd like to help out.

Notable current omissions: `Rc`, `Weak`, and most optimisation that doesn't affect the API.

## License

Licensed under either of

- Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING](CONTRIBUTING.md) for more information.

## [Code of Conduct](CODE_OF_CONDUCT.md)

## [Changelog](CHANGELOG.md)

## Versioning

`tiptoe` strictly follows [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) with the following exceptions:

- The minor version will not reset to 0 on major version changes (except for v1).  
Consider it the global feature level.
- The patch version will not reset to 0 on major or minor version changes (except for v0.1 and v1).  
Consider it the global patch level.

This includes the Rust version requirement specified above.  
Earlier Rust versions may be compatible, but this can change with minor or patch releases.

Which versions are affected by features and patches can be determined from the respective headings in [CHANGELOG.md](CHANGELOG.md).

Note that dependencies of this crate may have a more lenient MSRV policy!
Please use `cargo +nightly update -Z minimal-versions` in your automation if you don't generate Cargo.lock manually (or as necessary) and require support for a compiler older than current stable.
