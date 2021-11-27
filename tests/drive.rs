use flowync::{Flower, Leaper};

#[test]
fn drive() {
    let flower = Flower::<i32, String>::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        move || {
            handle.start_flowing();
            for i in 0..5 {
                println!("{}", i);
                handle.send_current(i);
            }
            handle.ok("Ok".to_string());
        }
    });

    let mut exit = false;
    let mut sum = 0;
    let mut received_last_value = 0;

    loop {
        flower.try_recv(
            |option| {
                if let Some(value) = option {
                    received_last_value = value;
                    sum += value;
                }
            },
            |result| {
                match result {
                    Ok(value) => {
                        assert_eq!("Ok", &value);
                    }
                    _ => (),
                }

                exit = true;
            },
        );

        if exit {
            break;
        }
    }

    assert_eq!(received_last_value, 4);
    assert_eq!(sum, 10);

    let leaper = Leaper::<String>::new(1);
    std::thread::spawn({
        let handle = leaper.handle();
        move || {
            handle.ok("Ok".to_string());
        }
    });

    let mut exit = false;

    loop {
        leaper.try_catch(|result| {
            match result {
                Ok(value) => {
                    assert_eq!("Ok", &value);
                }
                _ => (),
            }
            exit = true;
        });

        if exit {
            break;
        }
    }
}
