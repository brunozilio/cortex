//! Links a WhatsApp number in the terminal and prints what reaches it.
//!
//! `cargo run -p superfik-channels --example link [data dir]`: scan the QR
//! with WhatsApp › Linked devices › Link a device. The session is kept in
//! the data dir (default: `superfik-whatsapp-link` in the temp dir), so a second run reconnects
//! without a QR. Type `<number> <text>` and Enter to send; Ctrl-C to quit.
//! Nothing is answered on its own.

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Arc;

use qrcode::QrCode;
use qrcode::render::unicode::Dense1x2;
use superfik_channels::whatsapp::WhatsappOpener;
use superfik_channels::{ChannelEvent, Events, LinkSpec, Opener};
use superfik_core::ConnectionId;
use superfik_core::channel::ChannelKind;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_dir =
        std::env::args().nth(1).map_or_else(|| std::env::temp_dir().join("superfik-whatsapp-link"), PathBuf::from);
    let events: Events = Arc::new(|event| match event {
        ChannelEvent::Qr { code, valid_secs } => match QrCode::new(code.as_bytes()) {
            Ok(qr) => println!("\n{}\nScan within {valid_secs} s.", qr.render::<Dense1x2>().quiet_zone(true).build()),
            Err(error) => eprintln!("cannot draw the QR: {error}"),
        },
        ChannelEvent::Linked { account } => println!("Linked as {account}. Type `<number> <text>` to send."),
        ChannelEvent::File { message, media, bytes } => {
            println!("File for {message}: {:?} {:?}, {} bytes", media.kind, media.filename, bytes.0.len());
        }
        ChannelEvent::Message { message, delivery } => println!(
            "[{delivery:?}] {:?} in {}: {}{}",
            message.source,
            message.exchange,
            message.sender_name.map(|name| format!("{name}: ")).unwrap_or_default(),
            message.text
        ),
        ChannelEvent::Closed(reason) => println!("Closed: {reason:?}"),
    });
    let opener = WhatsappOpener::new()?;
    let spec = LinkSpec {
        connection: ConnectionId::new(),
        kind: ChannelKind::WhatsappWeb,
        data_dir,
        credentials: superfik_channels::Credentials::default(),
    };
    let channel = opener.open(&spec, events)?;
    for line in std::io::stdin().lock().lines() {
        let line = line?;
        let Some((to, text)) = line.split_once(' ') else { continue };
        match channel.send_text(to, text) {
            Ok(id) => println!("sent {id}"),
            Err(error) => eprintln!("not sent: {error}"),
        }
    }
    channel.close();
    Ok(())
}
