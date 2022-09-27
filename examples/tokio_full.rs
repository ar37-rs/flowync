// [dependencies]
// # Make sure to enable tokio "full" features (multithreaded support) like so:
// tokio = { version = "1", features = ["full"] }

// #![allow(clippy::needless_return)]

use flowync::Flower;
use std::{io::Error, time::Instant};

type TestTokioFlower = Flower<String, u32>;

#[tokio::main]
async fn main() {
    let instant: Instant = Instant::now();
    let flower: TestTokioFlower = Flower::new(1);
    tokio::spawn({
        let this = flower.handle();
        // Activate
        this.activate();
        async move {
            let id = this.id();
            let result =
                Ok::<String, Error>(format!("the flower with id: {} is flowing", id).into());

            match result {
                Ok(value) => {
                    // Send current flower progress.
                    this.send_async(value).await;
                }
                Err(e) => {
                    // Return error immediately if something not right, for example:
                    return this.error_verbose(e.into());
                }
            }

            // Explicit Cancelation example:
            // Check if the current flower should be canceled
            if this.should_cancel() {
                let value = format!("canceling the flower with id: {}", id);
                this.send_async(value).await;
                return this.error(format!("the flower with id: {} canceled", id));
            }

            // Finalizing
            // either `return this.success(instant.elapsed().subsec_micros());` or
            this.success(instant.elapsed().subsec_micros());
            // both are ok
        }
    });

    let mut done = false;

    loop {
        if flower.is_active() {
            flower
                .extract(|channel| {
                    // Poll channel
                    if let Some(value) = channel {
                        println!("{}", value);
                    }
                })
                .finalize(|result| {
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
