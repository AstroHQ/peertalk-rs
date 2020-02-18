# Cross-platform PeerTalk Implemented in Rust

This implements the ability to negotiate a network connection over USB to iOS devices via Apple's USB muxer. This can work across platforms assuming iTunes or Apple Mobile Supprot is present. May work with open source [usbmuxd/libimobiledevice](http://www.libimobiledevice.org/) on linux.

Based on [PeerTalk by Rasmus Andersson](https://github.com/rsms/peertalk)

## Status

- [x] Basic device listen protocol work started
- [x] macOS/linux UNIX domain socket support

## TODO

- [ ] Connect (network sockets) support
