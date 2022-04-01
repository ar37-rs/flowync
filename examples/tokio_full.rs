// [dependencies]
// # Make sure to enable tokio "full" features (multithreaded support) like so:
// tokio = { version = "1", features = ["full"] }

use flowync::Flower;
use std::{io::Error, time::Instant};

#[tokio::main]
async fn main() {
    let instant: Instant = Instant::now();
    let flower: Flower<String, u32> = Flower::new(1);
    tokio::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        async move {
            let id = handle.id();
            let result =
                Ok::<String, Error>(format!("the flower with id: {} is flowing", id).into());

            match result {
                Ok(value) => {
                    // Send current flower progress.
                    handle.send_async(value).await;
                }
                Err(e) => {
                    // Return error immediately if something not right, for example:
                    return handle.err(e.to_string());
                }
            }

            // Explicit Cancelation example:
            // Check if the current flower should be canceled
            if handle.should_cancel() {
                let value = format!("canceling the flower with id: {}", id);
                handle.send_async(value).await;
                return handle.err(format!("the flower with id: {} canceled", id));
            }

            return handle.ok(instant.elapsed().subsec_micros());
        }
    });

    let mut done = false;

    loop {
        if flower.is_active() {
            flower
                .try_recv(|channel| {
                    if let Some(value) = channel {
                        println!("{}\n", value);
                    }
                })
                .on_complete(|result| {
                    match result {
                        Ok(elapsed) => println!(
                            "the flower with id: {} finished in: {:?} microseconds \n",
                            flower.id(),
                            elapsed
                        ),
                        Err(err_msg) => println!("{}", err_msg),
                    }
                    done = true;
                });
        }

        // Cancel if need to
        // flower.cancel();

        if done {
            break;
        }
    }
}
