use peertalk::DeviceListener;

fn main() {
    let mut listener = DeviceListener::new().expect("Failed to create device listener");
    println!("Listening for iOS devices...");
    loop {
        let event = listener.recv_event().unwrap();
        println!("Event: {:?}", event);
    }
}
