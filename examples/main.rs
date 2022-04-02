#![allow(clippy::needless_return)]

use flowync::Flower;

fn main() {
    let flower = Flower::<i32, String>::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        move || {
            for i in 0..10 {
                // // Send current value through channel, will block the spawned thread
                // until the option value successfully being polled in the main thread.
                handle.send(i);
                // or handle.send_async(i).await; can be used from any multithreaded async runtime,

                // // Return error if the job is failure, for example:
                // if i >= 3 {
                //    return handle.err("Err");
                // }
            }
            // And return ok if the job successfully completed.
            return handle.ok("Ok".to_string());
        }
    });

    let mut exit = false;

    loop {
        // Instead of polling the mutex over and over, check if the flower is_active()
        // and will deactivate itself if the result value successfully received.
        // Note: this fn is non-blocking (won't block the current thread).
        if flower.is_active() {
            flower
                .try_recv(|channel| {
                    if let Some(value) = channel {
                        println!("{}", value);
                    }
                })
                .on_complete(|result| {
                    match result {
                        Ok(value) => println!("{}", value),
                        Err(err_msg) => println!("{}", err_msg),
                    }
                    exit = true;
                });
        }

        if exit {
            break;
        }
    }
}
