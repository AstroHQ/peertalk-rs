//! Crate to handle establishing network connections over USB to apple devices
#![forbid(missing_docs)]

use std::net::SocketAddr;
#[cfg(target_os = "windows")]
use std::net::TcpStream;
#[cfg(not(target_os = "windows"))]
use std::os::unix::net::UnixStream;

mod protocol;
use protocol::{Packet, PacketType, Protocol};

/// Listens for iOS devices connecting over USB via Apple Mobile Support/usbmuxd
pub struct DeviceListener {
    #[cfg(target_os = "windows")]
    socket: TcpStream,
    #[cfg(not(target_os = "windows"))]
    socket: UnixStream,
}
impl DeviceListener {
    #[cfg(not(target_os = "windows"))]
    fn connect_unix() -> UnixStream {
        UnixStream::connect("/var/run/usbmuxd").expect("Failed to connect to /var/run/usbmuxd")
    }
    #[cfg(target_os = "windows")]
    fn connect_windows() -> TcpStream {
        let addr: SocketAddr = "127.0.0.1:27015".parse().unwrap();
        TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(5))
            .expect("Failed to connect to USB mux service")
    }
    /// Produces a new device listener, registering with usbmuxd/apple mobile support service
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        let socket = Self::connect_windows();
        #[cfg(not(target_os = "windows"))]
        let socket = Self::connect_unix();
        let mut listener = DeviceListener { socket };
        listener.start_listen();
        listener
    }
    fn start_listen(&mut self) {
        let command = protocol::Command::new("Listen");
        let mut payload: Vec<u8> = Vec::new();
        plist::to_writer_xml(&mut payload, &command).unwrap();
        assert_ne!(payload.len(), 0, "Should have > 0 bytes payload");
        self.send_payload(PacketType::PlistPayload, Protocol::Plist, payload);
        println!("Payload sent, waiting for response...");
        let packet = Packet::from_reader(&mut self.socket).unwrap();
        // let result = protocol::ResultMessage::from(0);
        let cursor = std::io::Cursor::new(&packet.data[..]);
        let res = protocol::ResultMessage::from_reader(cursor);
        println!("Got result: {:?}", res);
        loop {
            let packet = Packet::from_reader(&mut self.socket).unwrap();
            // println!("Read: {:?}", packet);
            if packet.protocol == Protocol::Plist {
                let cursor = std::io::Cursor::new(&packet.data[..]);
                let msg = protocol::DeviceEventMessage::from_reader(cursor);
                println!("Payload message: {:?}", msg);
            } else {
                println!("Failed to get plist protocol message, ignoring");
            }

            // let s = std::str::from_utf8(&packet.data[..]).unwrap();
            // println!("Payload: {}", s);
        }
    }
    fn send_payload(&mut self, packet_type: PacketType, protocol: Protocol, payload: Vec<u8>) {
        let packet = Packet::new(protocol, packet_type, 0, payload);
        packet.write_into(&mut self.socket).unwrap();
    }
}
