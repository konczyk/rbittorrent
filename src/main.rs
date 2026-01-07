mod torrent;

use crate::torrent::download::Download;
use crate::torrent::torrent::Torrent;
use clap::{Parser, ValueEnum};
use std::path::Path;
use std::{fs, io};

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

fn main() -> io::Result<()> {
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
            fs::read(&args.torrent_file)
                .and_then(|cnt| {
                    let torrent = Torrent::new(cnt.as_slice());
                    let output_dir = args.output_dir.expect("Missing output dir flag");
                    let output_file = Path::new(&args.torrent_file).file_stem().and_then(|x| x.to_str()).unwrap_or("output").to_string();
                    let downloader = Download::new(&torrent, output_dir, output_file, args.debug);
                    match downloader.download() {
                        Ok(_) => (),
                        Err(e) => {
                            if args.debug {
                                eprintln!("Torrent download failed: {e}");
                            }
                        }
                    };
                    Ok(())
                })
        },
    }
}