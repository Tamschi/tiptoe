# tiptoe Changelog

<!-- markdownlint-disable no-trailing-punctuation -->

## 0.0.2

2021-11-12

- **Breaking changes**:
  - Renamed trait `TipToed` to `IntrusivelyCountable` and its function `tip_toe` to `ref_counter`.
  - Corrected signature of `IntrusivelyCountable::ref_counter` to use its associated type.

- Revisions:
  - Added `"no_std"` keyword to package.

## 0.0.1

2021-11-08

Initial unstable release
