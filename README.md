# Flowync

[![Crates.io](https://img.shields.io/crates/v/flowync.svg)](https://crates.io/crates/flowync)
[![Flowync documentation](https://docs.rs/flowync/badge.svg)](https://docs.rs/flowync)
[![CI](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/flowync/actions/workflows/ci.yml)

## Quick Example

```rust
use flowync::Flower;

fn main() {
    let flower = Flower::<i32, String>::new(1);
    let handle = flower.handle();
    std::thread::spawn(move || {
        handle.start_flowing();
        for i in 0..10 {
            handle.send_current(i);
            // // Return error.
            // if i >= 3 {
            //    return handle.err("Err".into());
            // }
        }
        handle.ok("Ok".into());
    });

    let mut exit = false;

    loop {
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
