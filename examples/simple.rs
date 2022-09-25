use flowync::Flower;
use std::io::Error;

type TestSimpleFlower = Flower<(), String>;

fn main() {
    let flower: TestSimpleFlower = Flower::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        move || {
            let id = handle.id();
            let result = Ok::<String, Error>(format!(
                "thse flower with id: {} successfully completed",
                id
            ));
            match result {
                Ok(value) => {
                    // And return if the job successfully completed.
                    handle.success(value);
                }
                Err(e) => {
                    // Return error immediately if something not right, for example:
                    handle.error(e);
                }
            }
        }
    });

    let mut exit = false;

    loop {
        // Check if the flower is_active()
        // and will deactivate itself if the result value successfully received.
        if flower.is_active() {
            flower.result(|result| {
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
