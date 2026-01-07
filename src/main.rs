mod torrent;

use clap::{Parser, ValueEnum};
use std::{fs, io};
use crate::torrent::torrent::Torrent;

#[derive(Debug, Clone, ValueEnum)]
#[value(rename_all = "snake_case")]
enum Command {
    Info,
    Download,
}

#[derive(Parser, Debug)]
struct Args {
    /// Command name
    #[arg(
        required = true,
    )]
    command: Command,

    /// A torrent file
    #[arg(
        required = true,
    )]
    torrent_file: String,

    /// Directory to save the file into
    #[arg(
        required = false,
    )]
    output_dir: Option<String>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let peer_id = "xwgeweorwehnrot34t29";

    match args.command {
        Command::Info => {
            fs::read(args.torrent_file)
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
        Command::Download=> {
            fs::read(args.torrent_file)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    let peers = torrent.get_peers(peer_id);
                    let pieces = torrent.count_pieces();
                    Ok(())
                })
        },
    }
}