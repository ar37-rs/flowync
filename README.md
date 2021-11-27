# Flowync

[![Crates.io](https://img.shields.io/crates/v/flowync.svg)](https://crates.io/crates/flowync)
[![Flowync documentation](https://docs.rs/flowync/badge.svg)](https://docs.rs/flowync)
[![CI](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml)

## Quick Example

```rust
use flowync::Flower;

fn main() {
    let flower = Flower::<i32, String>::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        move || {
            // Start flowing to initialize.
            handle.start_flowing();
            for i in 0..10 {
                // Send current value through channel, will block the spawned thread
                // until the option value successfully being polled in the main thread.
                handle.send_current(i);
                // // Return error if the job is failure, for example:
                // if i >= 3 {
                //    return handle.err("Err".to_string());
                // }
            }
            // And return ok if the job successfully completed.
            return handle.ok("Ok".to_string());
        }
    });

    let mut exit = false;

    loop {
        // This fn will deactivate itself if the result contains a value.
        // Note: this fn is non-blocking (won't block the current thread).
        flower.try_recv(
            |option| {
                if let Some(value) = option {
                    println!("{}", value);
                }
            },
            |result| {
                match result {
                    Ok(value) => {
                        println!("{}", value);
                    }
                    Err(e) => {
                        println!("{}", e);
                    }
                }
                exit = true;
            },
        );

        if exit {
            break;
        }
    }
}
```

## More examples
can be found here [here](https://github.com/Ar37-rs/flowync/tree/main/examples)