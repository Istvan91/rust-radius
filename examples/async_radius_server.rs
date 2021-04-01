//! An example on how to use RADIUS Server
//!
//! To run Async RADIUS Server example
//! ```bash
//! cargo run --example async_radius_server --all-features
//! ```


use radius_rust::protocol::dictionary::Dictionary;
use radius_rust::protocol::error::RadiusError;
use radius_rust::protocol::radius_packet::{ RadiusMsgType, TypeCode };
use radius_rust::tools::{ ipv6_string_to_bytes, ipv4_string_to_bytes, integer_to_bytes };

use log::{ debug, LevelFilter };
use simple_logger::SimpleLogger;


use async_std::task;
use radius_rust::servers::async_server::{ AsyncServer, AsyncServerBuilder };


// Define your own RADIUS packet handlers
// 
// Ideally, on success, each handler should return RADIUS packet, that would be send as a response to
// RADIUS client
// In case of an error, nothing would be sent to client (which isn't correct behaviour and should be
// fixed later)
fn handle_auth_request(server: &AsyncServer, request: &mut [u8]) -> Result<Vec<u8>, RadiusError> {
    let ipv6_bytes = ipv6_string_to_bytes("fc66::1/64")?;
    let ipv4_bytes = ipv4_string_to_bytes("192.168.0.1")?;

    let attributes = vec![
        server.create_attribute_by_name("Service-Type",       integer_to_bytes(2))?,
        server.create_attribute_by_name("Framed-IP-Address",  ipv4_bytes)?,
        server.create_attribute_by_name("Framed-IPv6-Prefix", ipv6_bytes)?
    ];

    let mut reply_packet = server.create_reply_packet(TypeCode::AccessAccept, attributes, request);
    Ok(reply_packet.to_bytes())
}

fn handle_acct_request(server: &AsyncServer, request: &mut [u8]) -> Result<Vec<u8>, RadiusError> {
    let ipv6_bytes        = ipv6_string_to_bytes("fc66::1/64")?;
    let ipv4_bytes        = ipv4_string_to_bytes("192.168.0.1")?;
    let nas_ip_addr_bytes = ipv4_string_to_bytes("192.168.1.10")?;

    let attributes = vec![
        server.create_attribute_by_name("Service-Type",       integer_to_bytes(2))?,
        server.create_attribute_by_name("Framed-IP-Address",  ipv4_bytes)?,
        server.create_attribute_by_name("Framed-IPv6-Prefix", ipv6_bytes)?,
        server.create_attribute_by_name("NAS-IP-Address",     nas_ip_addr_bytes)?
    ];

    let mut reply_packet = server.create_reply_packet(TypeCode::AccountingResponse, attributes, request);
    Ok(reply_packet.to_bytes())
}

fn handle_coa_request(server: &AsyncServer, request: &mut [u8]) -> Result<Vec<u8>, RadiusError> {
    let state = String::from("testing").into_bytes();

    let attributes = vec![
        server.create_attribute_by_name("State", state)?
    ];

    let mut reply_packet = server.create_reply_packet(TypeCode::CoAACK, attributes, request);
    Ok(reply_packet.to_bytes())
}
// ------------------------

fn main() -> Result<(), RadiusError> {
    SimpleLogger::new().with_level(LevelFilter::Debug).init().unwrap();
    debug!("Async RADIUS Server started");

    task::block_on(async {
        let dictionary    = Dictionary::from_file("./dict_examples/integration_dict")?;
        let allowed_hosts = vec![String::from("127.0.0.1")];

        let server = AsyncServerBuilder::with_dictionary(dictionary)
            .set_server(String::from("127.0.0.1"))
            .set_secret(String::from("secret"))
            .set_allowed_hosts(allowed_hosts)
            .add_protocol_port(RadiusMsgType::AUTH, 1812)
            .add_protocol_port(RadiusMsgType::ACCT, 1813)
            .add_protocol_port(RadiusMsgType::COA,  3799)
            .add_protocol_hanlder(RadiusMsgType::AUTH, handle_auth_request)
            .add_protocol_hanlder(RadiusMsgType::ACCT, handle_acct_request)
            .add_protocol_hanlder(RadiusMsgType::COA,  handle_coa_request)
            .build_server();

        server.run_server().await;
        Ok(())
    })
}