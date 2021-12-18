# Changelog

## [1.8.9] - 2021-12-19
- Feat(Flower/Leaper): fn result_is_ready and channel_is_present added
- parking-lot feature added (std sync mutex and std sync condivar replacement)
- a few optimizations.

## [1.8.1] - 2021-11-29
- Internal: rename SendAsync to AsyncSuspender
- Internal: add missing field inside impl Debug
- Add tokio_full.rs example.

## [1.8.0] - 2021-11-29
- Fix(Flower): polling mutex only if needed inside try_recv(|value| {...}, |result| {...}) which is introduced in version 1.0.0, now everyting's working as expected.
- Feat(FlowerHandle): send_async() support can be used from any async runtime.
- Feat(Flower): fn is_canceled() to check cancelation added.

## [1.0.0] - 2021-11-28
- Improvement(Flower): instead of polling the Mutex over and over, poll the mutex inside fn try_recv(|value| {...}, |result| {...}) only if needed.
- No longer need to call fn start_flowing at initialization.
- Remove a few unnecessary FNs.

## [0.2.0] - 2021-11-27
- Improvement(Leaper): Remove unnecessary polling the mutex inside fn try_catch(|result| {...}).
- Explicit cancelation support
- Add more examples
- A few optimizations.

## [0.1.5] - 2021-11-27
- Remove unnecessary async topping.
- Two more Functionalities added (fn is_flowing and fn is_leaping) to the handle. 
