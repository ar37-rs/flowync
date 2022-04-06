#![allow(clippy::needless_return)]

use flowync::Flower;
use std::io::Error;

type TestSimpleFlower = Flower<(), String>;

fn main() {
    let flower: TestSimpleFlower = Flower::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        move || {
            let id = handle.id();
            let result = Ok::<String, Error>(format!(
                "thse flower with id: {} successfully completed",
                id
            ));
            match result {
                Ok(value) => {
                    // And return ok if the job successfully completed.
                    return handle.ok(value);
                }
                Err(e) => {
                    // Return error immediately if something not right, for example:
                    return handle.err(e.to_string());
                }
            }
        }
    });

    let mut exit = false;

    loop {
        // Instead of polling the mutex over and over, check if the flower is_active()
        // and will deactivate itself if the result value successfully received.
        // Note: this fn is non-blocking (won't block the current thread).
        if flower.is_active() {
            flower.then(
                |_channel| (),
                |result| {
                    match result {
                        Ok(value) => println!("{}", value),
                        Err(err_msg) => println!("{}", err_msg),
                    }
                    exit = true;
                },
            );
        }

        if exit {
            break;
        }
    }
}
