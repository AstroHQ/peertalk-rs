use usbmux::DeviceListener;

fn main() {
    let listener = DeviceListener::new();
    println!("Listening for iOS devices...");
}
