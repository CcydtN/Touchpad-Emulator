use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Router,
};
use log::*;
use serde::Deserialize;
use std::net::*;
use std::sync::{Arc, Mutex};
use tokio;
use tracing_subscriber;
use usbip::UsbInterfaceHandler;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let handler = Arc::new(Mutex::new(
        Box::new(usbip::hid::UsbHidKeyboardHandler::new_keyboard())
            as Box<dyn usbip::UsbInterfaceHandler + Send>,
    ));
    let server = Arc::new(usbip::UsbIpServer::new_simulated(vec![
        usbip::UsbDevice::new(0).with_interface(
            usbip::ClassCode::HID as u8,
            0x00,
            0x00,
            "Test HID",
            vec![usbip::UsbEndpoint {
                address: 0x81,         // IN
                attributes: 0x03,      // Interrupt
                max_packet_size: 0x08, // 8 bytes
                interval: 10,
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
