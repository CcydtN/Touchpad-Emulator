use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use log::*;
use num::FromPrimitive;
use serde::Deserialize;
use std::net::*;
use std::sync::{Arc, Mutex};
use tokio;
use tracing_subscriber;
use usbip::UsbInterfaceHandler;
use usbip::{hid::HidDescriptorType, Direction, EndpointAttributes};

#[rustfmt::skip]
const REPORT_DESCRIPTOR:&[u8] = &[
    //TOUCH PAD input TLC
    0x05, 0x0d,                         // USAGE_PAGE (Digitizers)
    0x09, 0x05,                         // USAGE (Touch Pad)
    0xa1, 0x01,                         // COLLECTION (Application)
    0x09, 0x22,                         // USAGE (FINGER)
    0xa1, 0x02,                         // COLLECTION (Logical)
    0x15, 0x00,                         //       LOGICAL_MINIMUM (0)
    0x25, 0x01,                         //       LOGICAL_MAXIMUM (1)
    0x09, 0x47,                         //       USAGE (Confidence)
    0x09, 0x42,                         //       USAGE (Tip switch)
    0x95, 0x02,                         //       REPORT_COUNT (2)
    0x75, 0x01,                         //       REPORT_SIZE (1)
    0x81, 0x02,                         //       INPUT (Data,Var,Abs)
    // Padding, size 1 bit, count 6
    0x95, 0x06,                         //       REPORT_COUNT (6)
    0x81, 0x03,                         //       INPUT (Cnst,Var,Abs)
    // X, Y
    0x05, 0x01,                         //       USAGE_PAGE (Generic Desk..
    0x15, 0x00,                         //       LOGICAL_MINIMUM (0)
    0x26, 0xff, 0x0f,                   //       LOGICAL_MAXIMUM (4095)
    0x75, 0x10,                         //       REPORT_SIZE (16)
    0x55, 0x0e,                         //       UNIT_EXPONENT (-2)
    0x65, 0x13,                         //       UNIT(Inch,EngLinear)
    0x09, 0x30,                         //       USAGE (X)
    0x35, 0x00,                         //       PHYSICAL_MINIMUM (0)
    0x46, 0x90, 0x01,                   //       PHYSICAL_MAXIMUM (400)
    0x95, 0x01,                         //       REPORT_COUNT (1)
    0x81, 0x02,                         //       INPUT (Data,Var,Abs)
    0x46, 0x13, 0x01,                   //       PHYSICAL_MAXIMUM (275)
    0x09, 0x31,                         //       USAGE (Y)
    0x81, 0x02,                         //       INPUT (Data,Var,Abs)

    0xc0,                               // END_COLLECTION
    0xc0,                               // END_COLLECTION
];

struct UsbHidTouchpadHandler {
    pub report_descriptor: Vec<u8>,
}

impl UsbHidTouchpadHandler {
    fn new() -> Self {
        Self {
            report_descriptor: REPORT_DESCRIPTOR.to_vec(),
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
                _ => unimplemented!("hid request {:?}", setup),
            }
        } else {
            // interrupt transfer
            if let Direction::In = ep.direction() {
                // interrupt in
                // match self.state {
                //     UsbHidKeyboardHandlerState::Idle => {
                //         if let Some(report) = self.pending_key_events.pop_front() {
                //             let mut resp = vec![report.modifier, 0];
                //             resp.extend_from_slice(&report.keys);
                //             info!("HID key down");
                //             self.state = UsbHidKeyboardHandlerState::KeyDown;
                //             return Ok(resp);
                //         }
                //     }
                //     UsbHidKeyboardHandlerState::KeyDown => {
                //         let resp = vec![0; 6];
                //         info!("HID key up");
                //         self.state = UsbHidKeyboardHandlerState::Idle;
                //         return Ok(resp);
                //     }
                // }
            }
        }
        Ok(vec![])
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let handler: Arc<Mutex<Box<dyn usbip::UsbInterfaceHandler + Send>>> =
        Arc::new(Mutex::new(Box::new(UsbHidTouchpadHandler::new())));

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
            handler.clone(),
        ),
    ]));

    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 2024);
    tokio::spawn(usbip::server(addr, server));

    let app = Router::new()
        .route("/", get(root))
        .route("/send", get(key))
        .with_state(handler);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Deserialize)]
struct Params {
    key: char,
}

async fn root() -> StatusCode {
    StatusCode::OK
}

#[axum::debug_handler]
async fn key(
    Query(params): Query<Params>,
    State(handler): State<Arc<Mutex<Box<dyn UsbInterfaceHandler + Send>>>>,
) -> StatusCode {
    info!("{params:?}");
    let mut handler = handler.lock().unwrap();
    if let Some(hid) = handler
        .as_any()
        .downcast_mut::<usbip::hid::UsbHidKeyboardHandler>()
    {
        hid.pending_key_events
            .push_back(usbip::hid::UsbHidKeyboardReport::from_ascii(
                params.key.try_into().unwrap(),
            ));
    }

    StatusCode::OK
}
