use peertalk::DeviceListener;

fn main() {
    let listener = DeviceListener::new().expect("Failed to create device listener");
    println!("Listening for iOS devices...");
    loop {
        match listener.next_event() {
            Some(event) => println!("Event: {:?}", event),
            None => std::thread::sleep(std::time::Duration::from_secs(5)),
        }
    }
}
