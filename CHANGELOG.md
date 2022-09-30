# Changelog
## [5.0.1] 2022-10-1
- On Error: Support handling unexpected panic at runtime.
- Added CompactFlower, a Flower with composable error message (can be enable using `features = ["compact"]` )

## [4.8.0] - 2022-9-28
- Make Flower and Handle uncloneable to avoid any kind of deadlock and data races, added FlowerState as alternative.
- Internal only: Replace `Option<S, R>` with `TypeOpt<S, R>` managing value of the sync (mtx) state.
- Revert: don't use trait bound `Clone` on type `<S, R>` 
- Added functionality to set verboser error message using `fn error_verbose`.
- Remove deprecated fn
- Added 'set_result` and `try_result` fn for more simpler error handling.
- Added `IOError` type alias
- Update examples
- Doc and performance improved.

## [4.0.2] - 2022-9-26
- Fix unexpected deadlock on `result` fn.

## [4.0.1] - 2022-9-26
- Small Optimization
   * Added `IntoResult` trait to convert `Option<T>` into `Result<T, E>`

## [4.0.0] - 2022-9-25
- Refactor
   * `ok` fn changed to `success` fn, and `err` fn changed to `error` fn for more convenience.
   * Added `result` and `extract` fn
   * Remove `then` fn
   * Remove parking_lot as dependency.

## [3.0.0] - 2022-4-6
- Remove needless traits: Sync + 'static
- Breaking changes:
    * remove fn try_recv and fn on_complete to avoid potential UB in a particular case.

## [2.0.x] - 2022-4-1
- Breaking change:
    * remove Leaper
    * refactor flower
- New features:
    * add activate and is_active fn hopefully more intuitive for showing progress or loading indicator.

## [1.8.9] - 2021-12-19
- Feat(Flower/Leaper): fn result_is_ready and channel_is_present added
- parking-lot feature added (std sync mutex and std sync condivar replacement)
- a few optimizations.
 
