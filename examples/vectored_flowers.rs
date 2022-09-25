// #![allow(clippy::needless_return)]

use flowync::Flower;
use std::{
    io::Error,
    time::{Duration, Instant},
};

type TestVectoredFlower = Flower<String, u32>;

fn main() {
    let instant: Instant = Instant::now();
    let mut vec_opt_flowers = Vec::new();
    for i in 0..5 {
        let flower: TestVectoredFlower = Flower::new(i);
        std::thread::spawn({
            let this = flower.handle();
            // Activate if need too, actually needless on this context because we use Option
            // this.activate();
            move || {
                let id = this.id();

                // Panic if need to.
                // if id == 3 {
                //     std::panic::panic_any("loudness");
                // }

                let millis = id + 1;
                let sleep_dur = Duration::from_millis((10 * millis) as u64);
                std::thread::sleep(sleep_dur);
                let result =
                    Ok::<String, Error>(format!("the flower with id: {} wake up from sleep", id));
                match result {
                    Ok(value) => {
                        // Send current flower progress.
                        this.send(value);
                    }
                    Err(e) => {
                        // Return error immediately if something not right, for example:
                        return this.error(e);
                    }
                }

                // Explicit cancelation example:
                // Check if the current flower should be canceled
                if this.should_cancel() {
                    let value = format!("canceling the flower with id: {}", id);
                    this.send(value);
                    return this.error(format!("the flower with id: {} canceled", id));
                }

                // Finishing
                // either return this.ok(instant.elapsed().subsec_millis()); or
                this.success(instant.elapsed().subsec_millis());
                // both are ok
            }
        });
        vec_opt_flowers.push(Some(flower));
    }

    let num_flowers = vec_opt_flowers.len();
    let mut count_down = num_flowers;

    loop {
        vec_opt_flowers.iter_mut().for_each(|opt_flower| {
            if let Some(flower) = opt_flower {
                let id = flower.id();

                // Cancel if need to.
                // if (id % 2 != 0) || (id == 0) {
                //     flower.cancel();
                // }

                let mut done = false;
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
                                "the flower with id: {} finished in: {:?} milliseconds\n",
                                id, elapsed
                            ),
                            Err(err_msg) => println!("{}", err_msg),
                        }
                        done = true;
                    });

                if done {
                    // Set to None to free later
                    *opt_flower = None;
                    count_down -= 1;
                }
            }
        });

        if count_down == 0 {
            // Free completed flower
            vec_opt_flowers = vec_opt_flowers
                .into_iter()
                .filter(|opt_flower| opt_flower.is_some())
                .collect::<Vec<_>>();

            println!(
                "finished with vec_opt_flowers remains: {}",
                vec_opt_flowers.len()
            );
            break;
        }
    }
}
