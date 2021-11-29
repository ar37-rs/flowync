use flowync::Flower;
use std::{
    io::Error,
    time::{Duration, Instant},
};

fn main() {
    let instant: Instant = Instant::now();
    let mut vec_opt_flowers = Vec::new();
    for i in 0..5 {
        let flower: Flower<String, u32> = Flower::new(i);
        std::thread::spawn({
            let handle = flower.handle();
            move || {
                let id = handle.id();
                // // Panic if need to.
                // if id == 3 {
                //    std::panic::panic_any("loudness");
                // }

                let millis = id + 1;
                let sleep_dur = Duration::from_millis((10 * millis) as u64);
                std::thread::sleep(sleep_dur);
                let result = Ok::<String, Error>(
                    format!("the flower with id: {} wake up from sleep", id).into(),
                );
                match result {
                    Ok(value) => {
                        // Send current flower progress.
                        handle.send(value);
                    }
                    Err(e) => {
                        // Return error immediately if something not right, for example:
                        return handle.err(e.to_string());
                    }
                }

                // Explicit cancelation example:
                // Check if the current flower should be canceled
                if handle.should_cancel() {
                    let value = format!("canceling the flower with id: {}", id);
                    handle.send(value);
                    return handle.err(format!("the flower with id: {} canceled", id));
                }

                return handle.ok(instant.elapsed().subsec_millis());
            }
        });
        vec_opt_flowers.push(Some(flower));
    }

    let num_flowers = vec_opt_flowers.len();
    let mut count_down = num_flowers;

    loop {
        for i in 0..num_flowers {
            if let Some(flower) = &vec_opt_flowers[i] {
                let id = flower.id();
                // // Cancel if need to.
                // if (id % 2 != 0) || (id == 0) {
                //     flower.cancel();
                // }
                let mut done = false;
                flower.try_recv(
                    |value| {
                        println!("{}\n", value);
                    },
                    |result| {
                        match result {
                            Ok(elapsed) => println!(
                                "the flower with id: {} finished in: {:?} milliseconds\n",
                                id, elapsed
                            ),
                            Err(e) => println!("{}", e),
                        }
                        done = true;
                    },
                );

                if done {
                    vec_opt_flowers[i] = None;
                    count_down -= 1;
                }
            }
        }

        if count_down == 0 {
            break;
        }
    }
}
