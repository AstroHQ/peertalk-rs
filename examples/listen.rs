use peertalk::DeviceListener;
#[macro_use]
extern crate log;

fn main() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Trace)
        .init();
    let listener = DeviceListener::new().expect("Failed to create device listener");
    info!("Listening for iOS devices...");
    loop {
        match listener.next_event() {
            Some(event) => info!("Event: {:?}", event),
            None => std::thread::sleep(std::time::Duration::from_secs(5)),
        }
    }
}
