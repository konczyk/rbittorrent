mod bencode;
mod torrent;

use bencode::decode_bencoded_value;
use clap::{Parser, ValueEnum};
use std::net::TcpStream;
use std::{fs, io};
use torrent::Torrent;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "snake_case")]
enum Command {
    Decode,
    Info,
    Peers,
    Handshake,
}

#[derive(Parser, Debug)]
struct Args {
    /// Name of the command
    #[arg(
        value_name = "COMMAND",
        required = true,
    )]
    command: Command,

    /// Value to decode or a torrent file
    #[arg(
        value_name = "VALUE",
        required = true,
    )]
    value: String,

    /// peer
    #[arg(
        value_name = "PEER",
        required = false,
    )]
    peer: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let peer_id = "xwgeweorwehnrot34t29";

    match args.command {
        Command::Decode => {
            println!("{}", decode_bencoded_value(args.value.as_bytes()).0.to_json());
            Ok(())
        },
        Command::Info => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    println!("Tracker URL: {}", torrent.url);
                    println!("Length: {}", torrent.length);
                    println!("Info Hash: {:x}", torrent.info_hash);
                    println!("Piece Length: {}", torrent.piece_length);
                    println!("Piece Hashes:");
                    torrent.pieces.chunks(20).map(|x| hex::encode(x)).for_each(|x| println!("{x}"));
                    Ok(())
                })
        },
        Command::Peers => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    match torrent.get_peers(peer_id) {
                        Ok(v) => {
                            v.iter().for_each(|(ip, port)| {
                                println!("{ip}:{port}")
                            });
                            Ok(())
                        },
                        Err(e) => {
                            eprintln!("An error occurred: {e}");
                            Err(e)
                        }
                    }
                })
        },
        Command::Handshake => {
            fs::read(args.value)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    match args.peer {
                        Some(peer) => {
                            if let Ok(mut stream) = TcpStream::connect(&peer) {
                                match torrent.send_handshake(&mut stream, peer_id) {
                                    Ok(response) => println!("Peer ID: {}", hex::encode(&response[48..68])),
                                    Err(e) => {
                                        eprintln!("An error occurred: {e}");
                                    },
                                }
                            } else {
                                eprintln!("Couldn't connect to server...");
                            }
                        },
                        None => eprintln!("Missing peer param"),
                    }
                    Ok(())
                })
        },
    }
}