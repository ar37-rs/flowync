#![allow(clippy::needless_return)]
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
            return handle.success("Ok".to_string());
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
