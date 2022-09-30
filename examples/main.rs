use flowync::{
    error::{Cause, IOError},
    Flower,
};
type TestFlower = Flower<u32, String>;

fn fetch_things(id: usize) -> Result<String, IOError> {
    let result = Ok::<String, IOError>(format!(
        "the flower with id: {} successfully completed fetching.",
        id
    ));
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
                .poll(|channel| {
                    if let Some(value) = channel {
                        println!("{}", value);
                    }
                })
                .finalize(|result| {
                    match result {
                        Ok(value) => println!("{}", value),
                        Err(Cause::Suppose(msg)) => {
                            println!("{}", msg)
                        }
                        Err(Cause::Panicked(_msg)) => {
                            // Handle things if stuff unexpectedly panicked at runtime.
                        }
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
