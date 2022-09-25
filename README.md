# Flowync

[![Crates.io](https://img.shields.io/crates/v/flowync.svg)](https://crates.io/crates/flowync)
![minimum rustc 1.61.0](https://img.shields.io/badge/rustc-1.61.0-blue.svg)
[![Flowync documentation](https://docs.rs/flowync/badge.svg)](https://docs.rs/flowync)
[![CI](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)

## Quick Example

```rust
use flowync::Flower;
type TestFlower = Flower<u32, String>;

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

                // Return error if the job is failure, for example:
                // if i >= 3 {
                //    return handle.error("Err");
                // }
            }
            // And return if the job successfully completed.
            handle.success("Ok".to_string());
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
can be found here [here](https://github.com/Ar37-rs/flowync/tree/main/examples)
