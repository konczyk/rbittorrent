mod torrent;

use crate::torrent::download::{Download, DownloadState};
use crate::torrent::torrent::Torrent;
use clap::{Parser, ValueEnum};
use std::path::Path;
use std::{fs, io};
use std::fs::File;
use std::sync::{Arc, Mutex};

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

    /// Run in debug mode
    #[arg(
        short,
        long
    )]
    debug: bool,

    /// Directory to save the file into
    #[arg(
        short,
        long
    )]
    output_dir: Option<String>,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

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
            let cnt = fs::read(&args.torrent_file)?;
            let torrent = Torrent::new(cnt.as_slice());
            let output_dir = args.output_dir.expect("Missing output dir flag");
            let pieces = torrent.count_pieces() as usize;
            let length = torrent.length as usize;
            let output_file = Path::new(&args.torrent_file).file_stem().and_then(|x| x.to_str()).unwrap_or("output").to_string();
            let final_path = format!("{}/{}", &output_dir, &output_file);
            let file = File::create(&final_path)?;
            file.set_len(torrent.length as u64)?;
            let downloader = Arc::new(
                Download::new(
                    torrent,
                    Arc::new(Mutex::new(DownloadState::new(pieces, length))),
                    Arc::new(Mutex::new(file)),
                    args.debug,
                )
            );
            match downloader.download().await {
                Ok(_) => (),
                Err(e) => {
                    if args.debug {
                        eprintln!("Torrent download failed: {e}");
                    }
                }
            };
            Ok(())
        },
    }
}