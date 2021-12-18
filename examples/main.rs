use flowync::Leaper;

fn main() {
    let leaper = Leaper::<String>::new(1);
    std::thread::spawn({
        let handle = leaper.handle();
        move || {
            // // Return error if the job is failure, for example:
            // return handle.err("Err".to_string());

            // And return ok if the job successfully completed.
            return handle.ok("Ok".to_string());
        }
    });

    let mut exit = false;

    loop {
        // Starting from version 0.2.x instead of polling the mutex over and over,
        // the fn will be activated automatically if the handle return a value
        // and will deactivate itself if the result value successfully catched.
        // Note: this fn is non-blocking (won't block the current thread).
        leaper.try_catch(|result| {
            match result {
                Ok(value) => {
                    println!("{}", value);
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
            exit = true;
        });
        leaper.cancel();

        if exit {
            break;
        }
    }
}
