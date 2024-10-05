use axum::{
    extract::{self, State},
    http::StatusCode,
    routing::post,
    Router,
};
use log::*;
use num::{traits::ToBytes, FromPrimitive, ToPrimitive};
use serde::Deserialize;
use std::{collections::HashMap, net::*};
use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};
use tokio;
use tower_http::services::ServeDir;
use tracing_subscriber;
use usbip::UsbInterfaceHandler;
use usbip::{hid::HidDescriptorType, Direction, EndpointAttributes};

#[rustfmt::skip]
// report from hid.git(linux) atmel_03eb_211c
const REPORT_DESCRIPTOR:&[u8] = &[
0x05, 0x0D,        // Usage Page (Digitizer)
0x09, 0x04,        // Usage (Touch Screen)
0xA1, 0x01,        // Collection (Application)
0x85, 0x01,        //   Report ID (1)
0x09, 0x22,        //   Usage (Finger)
0xA1, 0x00,        //   Collection (Physical)
0x09, 0x42,        //     Usage (Tip Switch)
0x15, 0x00,        //     Logical Minimum (0)
0x25, 0x01,        //     Logical Maximum (1)
0x75, 0x01,        //     Report Size (1)
0x95, 0x01,        //     Report Count (1)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x09, 0x32,        //     Usage (In Range)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x09, 0x37,        //     Usage (Data Valid)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x25, 0x1F,        //     Logical Maximum (31, Contact Identifier)
0x75, 0x05,        //     Report Size (5)
0x09, 0x51,        //     Usage (0x51)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x05, 0x01,        //     Usage Page (Generic Desktop Ctrls)
0x55, 0x0E,        //     Unit Exponent (-2)
0x65, 0x11,        //     Unit (System: SI Linear, Length: Centimeter)
0x35, 0x00,        //     Physical Minimum (0)
0x75, 0x10,        //     Report Size (16)
0x46, 0x58, 0x02,  //     Physical Maximum (600)
0x26, 0xFF, 0x0F,  //     Logical Maximum (4095)
0x09, 0x30,        //     Usage (X)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x46, 0x58, 0x02,  //     Physical Maximum (600)
0x26, 0xFF, 0x0F,  //     Logical Maximum (4095)
0x09, 0x31,        //     Usage (Y)
0x81, 0x02,        //     Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)

0x05, 0x0D,        //     Usage Page (Digitizer)
0x75, 0x08,        //     Report Size (8)
0x85, 0x02,        //     Report ID (2)
0x09, 0x55,        //     Usage (0x55)
0x25, 0x10,        //     Logical Maximum (16)
0xB1, 0x02,        //     Feature (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
0xC0,              //   End Collection
0xC0,              // End Collection
];

struct UsbHidTouchpadHandler {
    pub report_descriptor: Vec<u8>,
    pub max_contact_count: u8,
    pub next_idx: u8,
    pub slot: Vec<Option<(u16, u16)>>,
    pub map: HashMap<i64, usize>,
}

impl UsbHidTouchpadHandler {
    fn new() -> Self {
        let max_contact_count = 4;
        Self {
            report_descriptor: REPORT_DESCRIPTOR.to_vec(),
            max_contact_count,
            next_idx: 0,
            slot: vec![None; max_contact_count.to_usize().unwrap()],
            map: HashMap::new(),
        }
    }
}

impl UsbInterfaceHandler for UsbHidTouchpadHandler {
    fn get_class_specific_descriptor(&self) -> Vec<u8> {
        vec![
            0x09,                                     // bLength
            usbip::hid::HidDescriptorType::Hid as u8, // bDescriptorType: HID
            0x11,
            0x01,                                        // bcdHID 1.11
            0x00,                                        // bCountryCode
            0x01,                                        // bNumDescriptors
            usbip::hid::HidDescriptorType::Report as u8, // bDescriptorType[0] HID
            self.report_descriptor.len() as u8,
            (self.report_descriptor.len() >> 8) as u8, // wDescriptorLength[0]
        ]
    }

    fn handle_urb(
        &mut self,
        interface: &usbip::UsbInterface,
        ep: usbip::UsbEndpoint,
        transfer_buffer_length: u32,
        setup: usbip::SetupPacket,
        req: &[u8],
    ) -> std::io::Result<Vec<u8>> {
        if ep.is_ep0() {
            // control transfers
            match (setup.request_type, setup.request) {
                (0b10000001, 0x06) => {
                    // GET_DESCRIPTOR
                    // high byte: type
                    match FromPrimitive::from_u16(setup.value >> 8) {
                        Some(HidDescriptorType::Report) => {
                            return Ok(self.report_descriptor.clone());
                        }
                        _ => unimplemented!("hid descriptor {:?}", setup),
                    }
                }
                (0b00100001, 0x0A) => {
                    // SET_IDLE
                    return Ok(vec![]);
                }
                (0b0010100001, 0x01) => {
                    // GET_REPORT,
                    match FromPrimitive::from_u16(setup.value >> 8) {
                        Some(0x03) => {
                            // FEATURE REPORT
                            return Ok(vec![0x02, self.max_contact_count]);
                        }
                        _ => unimplemented!("hid descriptor {:?}", setup),
                    }
                }
                _ => unimplemented!("hid request {:?}", setup),
            }
        } else {
            // interrupt transfer
            if let Direction::In = ep.direction() {
                let mut buffer = vec![0x01];
                let item = self.slot[self.next_idx.to_usize().unwrap()];
                match item {
                    Some(x) => {
                        buffer.push(self.next_idx.to_le() << 3 | 0b111);
                        for &i in x.0.to_le_bytes().iter().chain(x.1.to_le_bytes().iter()) {
                            buffer.push(i);
                        }
                    }
                    None => {
                        buffer.push(self.next_idx.to_le() << 3);
                        for _ in 0..4 {
                            buffer.push(0);
                        }
                    }
                }
                return Ok(buffer);
            }
        }
        Ok(vec![])
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Clone)]
struct UsbInterface {
    usb_interface: Arc<Mutex<Box<dyn usbip::UsbInterfaceHandler + Send>>>,
}

impl Deref for UsbInterface {
    type Target = Arc<Mutex<Box<dyn usbip::UsbInterfaceHandler + Send>>>;

    fn deref(&self) -> &Self::Target {
        &self.usb_interface
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let shared_obj = UsbInterface {
        usb_interface: Arc::new(Mutex::new(Box::new(UsbHidTouchpadHandler::new()))),
    };

    let server = Arc::new(usbip::UsbIpServer::new_simulated(vec![
        usbip::UsbDevice::new(0).with_interface(
            usbip::ClassCode::HID as u8,
            0x00,
            0x00,
            "Test HID",
            vec![usbip::UsbEndpoint {
                address: 0x81, // IN
                attributes: EndpointAttributes::Interrupt as u8,
                max_packet_size: 0x08, // 8 bytes
                interval: 0xA,
            }],
            shared_obj.usb_interface.clone(),
        ),
    ]));

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2024);
    tokio::spawn(usbip::server(addr, server));

    let app = Router::new()
        .nest_service(
            "/",
            ServeDir::new("assets").append_index_html_on_directories(true),
        )
        .route("/touchstart", post(touch_start))
        .route("/touchmove", post(touch_move))
        .route("/touchend", post(touch_end))
        .route("/touchcancel", post(touch_cancel))
        .with_state(shared_obj);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
struct Touch {
    identifier: i64,
    x: u16,
    y: u16,
}

#[derive(Debug, Deserialize)]
struct Payload {
    touches: Vec<Touch>,
}

#[axum::debug_handler]
async fn touch_start(
    State(usb_interface): State<UsbInterface>,
    extract::Json(payload): extract::Json<Payload>,
) -> StatusCode {
    let mut handler = usb_interface.lock().unwrap();
    let touchpad_handler = handler
        .as_any()
        .downcast_mut::<UsbHidTouchpadHandler>()
        .unwrap();
    for touch in payload.touches {
        if let Some(idx) = touchpad_handler.slot.iter().position(|x| x.is_none()) {
            touchpad_handler.map.insert(touch.identifier, idx);
            let _ = touchpad_handler.slot[idx].insert((touch.x, touch.y));
        }
    }
    StatusCode::OK
}

#[axum::debug_handler]
async fn touch_move(
    State(usb_interface): State<UsbInterface>,
    extract::Json(payload): extract::Json<Payload>,
) -> StatusCode {
    let mut handler = usb_interface.lock().unwrap();
    let touchpad_handler = handler
        .as_any()
        .downcast_mut::<UsbHidTouchpadHandler>()
        .unwrap();
    for touch in payload.touches {
        if let Some(&idx) = touchpad_handler.map.get(&touch.identifier) {
            let _ = touchpad_handler.slot[idx].insert((touch.x, touch.y));
        }
    }
    StatusCode::OK
}

#[axum::debug_handler]
async fn touch_end(
    State(usb_interface): State<UsbInterface>,
    extract::Json(payload): extract::Json<Payload>,
) -> StatusCode {
    let mut handler = usb_interface.lock().unwrap();
    let touchpad_handler = handler
        .as_any()
        .downcast_mut::<UsbHidTouchpadHandler>()
        .unwrap();
    for touch in payload.touches {
        if let Some(&idx) = touchpad_handler.map.get(&touch.identifier) {
            let _ = touchpad_handler.slot[idx].take();
        }
        touchpad_handler.map.remove(&touch.identifier);
    }
    StatusCode::OK
}

#[axum::debug_handler]
async fn touch_cancel(
    State(usb_interface): State<UsbInterface>,
    extract::Json(payload): extract::Json<Payload>,
) -> StatusCode {
    let mut handler = usb_interface.lock().unwrap();
    let touchpad_handler = handler
        .as_any()
        .downcast_mut::<UsbHidTouchpadHandler>()
        .unwrap();
    for touch in payload.touches {
        if let Some(&idx) = touchpad_handler.map.get(&touch.identifier) {
            let _ = touchpad_handler.slot[idx].take();
        }
        touchpad_handler.map.remove(&touch.identifier);
    }
    StatusCode::OK
}
