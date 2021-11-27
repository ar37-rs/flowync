use flowync::Leaper;
use std::time::{Duration, Instant};

fn main() {
    let instant: Instant = Instant::now();
    let mut vec_opt_leapers = Vec::new();
    for i in 0..5 {
        let leaper: Leaper<u32> = Leaper::new(i);
        std::thread::spawn({
            let handle = leaper.handle();
            move || {
                let id = handle.id();
                // // Panic if need to.
                // if id == 3 {
                //    std::panic::panic_any("loudness");
                // }

                let millis = id + 1;
                let sleep_dur = Duration::from_millis((10 * millis) as u64);
                std::thread::sleep(sleep_dur);

                // Explicit cancelation example:
                if handle.should_cancel() {
                    return handle.err(format!("the leaper with id: {} canceled", id));
                }
                return handle.ok(instant.elapsed().subsec_millis());
            }
        });
        vec_opt_leapers.push(Some(leaper));
    }

    let num_leapers = vec_opt_leapers.len();
    let mut count_down = num_leapers;

    loop {
        for i in 0..num_leapers {
            if let Some(leaper) = &vec_opt_leapers[i] {
                let id = leaper.id();
                let mut done = false;
                // Cancel if need to.
                // if (id % 2 != 0) || (id == 0) {
                //     leaper.cancel();
                // }

                leaper.try_catch(|result| {
                    match result {
                        Ok(elapsed) => {
                            println!(
                                "the leaper with id: {} finished in: {:?} milliseconds\n",
                                id, elapsed
                            );
                        }
                        Err(e) => {
                            println!("{}", e);
                        }
                    }
                    done = true;
                });

                if done {
                    vec_opt_leapers[i] = None;
                    count_down -= 1;
                }
            }
        }

        if count_down == 0 {
            break;
        }
    }
}
