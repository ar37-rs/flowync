# Flowync

[![Crates.io](https://img.shields.io/crates/v/flowync.svg)](https://crates.io/crates/flowync)
![minimum rustc 1.61.0+](https://img.shields.io/badge/rustc-1.60.0+-blue.svg)
[![CI](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml)
[![Flowync documentation](https://docs.rs/flowync/badge.svg)](https://docs.rs/flowync)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)



## Quick Example

```rust
use flowync::{Flower, IOError};
type TestFlower = Flower<u32, String>;

fn fetch_things(id: usize) -> Result<String, IOError> {
    let result =
        Ok::<String, IOError>(format!("the flower with id: {} successfully completed fetching.", id));
    let success = result?;
    Ok(success)
}

fn main() {
    let flower: TestFlower = Flower::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        move || {
            for i in 0..10 {
                // Send current value through channel, will block the spawned thread
                // until the option value successfully being polled in the main thread.
                handle.send(i);
                // or handle.send_async(i).await; can be used from any multithreaded async runtime,
            }
            let result = fetch_things(handle.id());
            // Set result and then extract later.
            handle.set_result(result)
        }
    });

    let mut exit = false;

    loop {
        // Check if the flower is_active()
        // and will deactivate itself if the result value successfully received.
        if flower.is_active() {
            // another logic goes here...
            // e.g:
            // notify_loading_fn();

            flower
                .extract(|channel| {
                    // Poll channel
                    if let Some(value) = channel {
                        println!("{}", value);
                    }
                })
                .finalize(|result| {
                    match result {
                        Ok(value) => println!("{}", value),
                        Err(err_msg) => println!("{}", err_msg),
                    }
                    // Exit if finalized
                    exit = true;
                });
        }

        if exit {
            break;
        }
    }
}
```

## More examples
can be found here: [examples](https://github.com/Ar37-rs/flowync/tree/main/examples) and here: [eframe tokio (sync/async) integration example](https://github.com/ar37-rs/eframe_tokio_app)
