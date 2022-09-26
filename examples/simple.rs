use flowync::{Flower, IOError};
type TestSimpleFlower = Flower<(), String>;

fn fetch_things(id: usize) -> Result<String, IOError> {
    let result =
        Ok::<String, IOError>(format!("the flower with id: {} successfully completed", id));
    let success = result?;
    Ok(success)
}

fn main() {
    let flower: TestSimpleFlower = Flower::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        // Activate
        handle.activate();
        move || {
            let id = handle.id();
            let result = fetch_things(id);
            // Set result and then try_result later.
            handle.set_result(result)
        }
    });

    let mut exit = false;

    loop {
        // Check if the flower is_active()
        // and will deactivate itself if the result value successfully received.
        if flower.is_active() {
            flower.try_result(|result| {
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
