# Cross-platform PeerTalk Implemented in Rust

This implements the ability to negotiate a network connection over USB to iOS devices via Apple's USB muxer. This can work across platforms assuming iTunes or Apple Mobile Supprot is present. May work with open source [usbmuxd/libimobiledevice](http://www.libimobiledevice.org/) on linux, but is untested.

Based on [PeerTalk by Rasmus Andersson](https://github.com/rsms/peertalk)

## Usage

This just provides the necessary code for the host (mac/windows) side to detect an iPad/iPhone & negotiate a connection to the device if it's listening.

1. iOS app sets up a TCP listener on a known port
2. Host app uses peertalk to wait for device to be plugged in
3. Upon plug, tell peertalk to establish a connection to the device with the port used in step 1
4. You'll have a ready to use `TcpStream` upon success

## Status

- [x] Basic device listen protocol work started
- [x] macOS/linux UNIX domain socket support
- [x] Connect (network sockets) support

## TODO

- [ ] Improved error handling (`thiserror`)
