# Changelog

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
 
