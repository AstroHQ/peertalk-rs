use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use peertalk::connect_to_device;
use std::error::Error;
use std::fmt;
use std::io::{Error as IoError, Read, Write};

const PT_VERSION: u32 = 1;
const PT_FRAME_TYPE_DEVICE_INFO: u32 = 100;
const PT_FRAME_TYPE_TEXT_MSG: u32 = 101;
const PT_FRAME_TYPE_PING: u32 = 102;
const PT_FRAME_TYPE_PONG: u32 = 103;

fn main() {
    let mut socket = connect_to_device(3, 2345).expect("Failed to create device connection");
    // say hi
    let hi = PTFrame::text("Hello from Rust!");
    hi.write_into(&mut socket).unwrap();
    loop {
        // wait for data from device
        match PTFrame::from_reader(&mut socket) {
            Ok(frame) => process_frame(frame),
            Err(e) => println!("Error reading frame: {}", e),
        }
    }
    // std::thread::sleep(std::time::Duration::from_secs(5));
}
fn process_frame(frame: PTFrame) {
    // print out text if it's device info or text msg type
    if frame.frame_type == PT_FRAME_TYPE_DEVICE_INFO {
        // binary plist?
        let reader = std::io::Cursor::new(frame.payload);
        let info: plist::Value = plist::Value::from_reader(reader).unwrap();
        println!("Got device info: {:?}", info);
    } else if frame.frame_type == PT_FRAME_TYPE_TEXT_MSG {
        if let Ok(string) = std::str::from_utf8(&frame.payload[..]) {
            println!("Got text payload: {}", string);
        } else {
            println!("Failed to read payload of {} bytes", frame.payload.len());
        }
    } else if frame.frame_type == PT_FRAME_TYPE_PING {
        println!("Ping!");
    } else if frame.frame_type == PT_FRAME_TYPE_PONG {
        println!("Pong!");
    }
}
// peertalk frame example protocol

#[derive(Debug)]
pub enum FrameError {
    IoError(IoError),
}
impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FrameError::IoError(e) => write!(f, "IoError: {}", e),
        }
    }
}
impl Error for FrameError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            FrameError::IoError(e) => Some(e),
        }
    }
}
impl From<IoError> for FrameError {
    fn from(error: IoError) -> Self {
        FrameError::IoError(error)
    }
}

type Result<T> = ::std::result::Result<T, FrameError>;
#[derive(Debug)]
struct PTFrame {
    version: u32,
    frame_type: u32,
    tag: u32,
    // payload_size: u32,
    payload: Vec<u8>,
}
impl PTFrame {
    fn text(text: &str) -> PTFrame {
        /*typedef struct _PTExampleTextFrame {
        uint32_t length;
        uint8_t utf8text[0];
        } PTExampleTextFrame;*/
        let mut payload = Vec::with_capacity(text.len() + 4);
        payload.write_u32::<BigEndian>(text.len() as u32).unwrap();
        payload.write_all(text.as_bytes()).unwrap();
        PTFrame {
            version: PT_VERSION,
            frame_type: PT_FRAME_TYPE_TEXT_MSG,
            tag: 0,
            payload,
        }
    }
    fn write_into<W>(&self, writer: &mut W) -> Result<()>
    where
        W: Write,
    {
        writer.write_u32::<BigEndian>(self.version).unwrap();
        writer.write_u32::<BigEndian>(self.frame_type).unwrap();
        writer.write_u32::<BigEndian>(self.tag).unwrap();
        writer
            .write_u32::<BigEndian>(self.payload.len() as u32)
            .unwrap();
        writer.write_all(&self.payload[..]).unwrap();
        Ok(())
    }
    fn from_reader<R>(reader: &mut R) -> Result<Self>
    where
        R: Read,
    {
        let version = reader.read_u32::<BigEndian>()?;
        let frame_type = reader.read_u32::<BigEndian>()?;
        let tag = reader.read_u32::<BigEndian>()?;
        let payload_size = reader.read_u32::<BigEndian>()?;
        let payload = if payload_size > 0 {
            let mut payload = vec![0; payload_size as usize];
            reader.read_exact(&mut payload)?;
            payload
        } else {
            vec![]
        };
        let frame = PTFrame {
            version,
            frame_type,
            tag,
            payload,
        };
        Ok(frame)
    }
}
