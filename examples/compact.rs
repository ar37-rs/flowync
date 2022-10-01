use flowync::{
    error::{Compact, IOError},
    CompactFlower,
};

// Fix clippy warning.
#[allow(dead_code)]
enum ErrMessage {
    Segfault(String),
    Other,
}

type TestCompactFlower = CompactFlower<u32, String, ErrMessage>;

fn fetch_things(id: usize) -> Result<String, IOError> {
    let result = Ok::<String, IOError>(format!(
        "the compact flower with id: {} successfully completed fetching.",
        id
    ));
    let success = result?;
    Ok(success)
}

fn main() {
    let compact_flower: TestCompactFlower = CompactFlower::new(1);
    std::thread::spawn({
        let handle = compact_flower.handle();
        // Activate
        handle.activate();
        move || {
            for i in 0..10 {
                // Send current value through channel, will block the spawned thread
                // until the option value successfully being polled in the main thread.
                handle.send(i);
                // or handle.send_async(i).await; can be used from any multithreaded async runtime,
            }

            match fetch_things(handle.id()) {
                Ok(value) => {
                    handle.success(value);
                }
                Err(e) => handle.error(ErrMessage::Segfault(e.to_string())),
            }
        }
    });

    let mut exit = false;

    loop {
        // Check if the compact flower is_active()
        // and will deactivate itself if the result value successfully received.
        if compact_flower.is_active() {
            // another logic goes here...
            // e.g:
            // notify_loading_fn();

            compact_flower
                .poll(|channel| {
                    if let Some(value) = channel {
                        println!("{}", value);
                    }
                })
                .finalize(|result| {
                    match result {
                        Ok(value) => println!("{}", value),
                        Err(Compact::Suppose(ErrMessage::Segfault(msg))) => {
                            println!("{}", msg)
                        }
                        Err(Compact::Suppose(err_msg)) => {
                            if let ErrMessage::Other = err_msg {
                                // Handle if any
                            }
                        }
                        Err(Compact::Panicked(_msg)) => {
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
