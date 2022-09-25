use flowync::Flower;

#[test]
fn drive() {
    let flower = Flower::<i32, String>::new(1);
    std::thread::spawn({
        let handle = flower.handle();
        handle.activate();
        move || {
            for i in 1..20 {
                handle.send(i);
            }
            handle.error("Ok".to_string());
        }
    });

    let mut exit = false;
    let mut received_last_value = 0;

    loop {
        if flower.is_active() {
            flower
                .extract(|channel| {
                    if let Some(value) = channel {
                        received_last_value = value;
                        println!("{}", value);
                    }
                })
                .finalize(|result| {
                    match result {
                        Ok(value) => {
                            assert_eq!("Ok", &value);
                        }
                        _ => (),
                    }

                    exit = true;
                });
        }

        if exit {
            break;
        }
    }

    assert_eq!(received_last_value, 19);
}
