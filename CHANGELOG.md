# Changelog

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
