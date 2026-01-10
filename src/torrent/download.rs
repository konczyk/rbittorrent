use crate::torrent::torrent::Torrent;
use indicatif::{ProgressBar, ProgressStyle};
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io;
use std::io::{ErrorKind, Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

pub struct PeerSession {
    pub bitfield: Vec<u8>,
    pub choked: bool,
}

impl PeerSession {
    pub fn has_piece(&self, piece_index: usize) -> bool {
        let byte_index = piece_index / 8;
        let bit_index = piece_index % 8;
        self.bitfield.get(byte_index)
            .map(|&byte| (byte >> (7 - bit_index) & 1) != 0)
            .unwrap_or(false)
    }

    pub fn update_have(&mut self, piece_index: usize) {
        let byte_index = piece_index / 8;
        let bit_index = piece_index % 8;
        if byte_index >= self.bitfield.len() {
            self.bitfield.resize(byte_index + 1, 0);
        }
        self.bitfield[byte_index] |= 1 << (7 - bit_index);
    }
}

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

impl TryFrom<u8> for MessageId {
    type Error = io::Error;

    fn try_from(value: u8) -> Result<MessageId, Self::Error> {
        match value {
            0 => Ok(Self::Choke),
            1 => Ok(Self::Unchoke),
            2 => Ok(Self::Interested),
            3 => Ok(Self::NotInterested),
            4 => Ok(Self::Have),
            5 => Ok(Self::Bitfield),
            6 => Ok(Self::Request),
            7 => Ok(Self::Piece),
            8 => Ok(Self::Cancel),
            _ => Err(io::Error::new(io::ErrorKind::InvalidData, format!("Unknown message ID: {}", value))),
        }
    }
}

pub struct DownloadState {
    pub completed: Vec<u8>,
    pub in_progress: Vec<u8>,
    pub progress_bar: ProgressBar,
}

impl DownloadState {
    pub fn new(count: usize, length: usize) -> Self {
        let pb = ProgressBar::new(length as u64);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta}) {msg}")
            .unwrap()
            .progress_chars("#>-"));
        Self {
            completed: vec![0; (count as f64 / 8f64).ceil() as usize],
            in_progress: vec![0; (count as f64 / 8f64).ceil() as usize],
            progress_bar: pb,
        }
    }

    pub fn count_complete(&self) -> usize {
        self.completed.iter().map(|byte| byte.count_ones() as usize).sum()
    }

    pub fn is_complete(&self, total: usize) -> bool {
        self.count_complete() == total
    }

    pub fn is_piece_complete(&self, index: usize) -> bool {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        self.completed.get(byte_idx)
            .map(|&byte| (byte >> (7 - bit_idx) & 1) != 0)
            .unwrap_or(false)
    }

    pub fn is_piece_in_progress(&self, index: usize) -> bool {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        self.in_progress.get(byte_idx)
            .map(|&byte| (byte >> (7 - bit_idx) & 1) != 0)
            .unwrap_or(false)
    }

    pub fn set_complete(&mut self, index: usize) {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        if byte_idx < self.completed.len() {
            self.completed[byte_idx] |= 1 << (7 - bit_idx);
            self.in_progress[byte_idx] &= !(1 << (7 - bit_idx));
        }
    }

    pub fn set_in_progress(&mut self, index: usize, value: bool) {
        let byte_idx = index / 8;
        let bit_idx = index % 8;
        if byte_idx < self.in_progress.len() {
            if value {
                self.in_progress[byte_idx] |= 1 << (7 - bit_idx);
            } else {
                self.in_progress[byte_idx] &= !(1 << (7 - bit_idx));
            }
        }
    }
}

pub struct Download {
    torrent: Torrent,
    peer_id: String,
    state: Arc<Mutex<DownloadState>>,
    file: Arc<Mutex<File>>,
    debug: bool,
}

impl Download {
    const MAX_REQUESTS: u32 = 10;
    const BLOCK_SIZE: u32 = 16384;

    pub fn new(torrent: Torrent, state: Arc<Mutex<DownloadState>>, file: Arc<Mutex<File>>, debug: bool) -> Download {
        Download { torrent, peer_id: "avknevkjn43t34tn389f".to_string(), state, file, debug }
    }

    fn debug_err(&self, msg: &str, e: io::Error) -> io::Error {
        if self.debug {
            eprintln!("{msg} {}", e);
        }
        e
    }

    fn debug(&self, msg: &str) {
        if self.debug {
            println!("{msg}");
        }
    }

    async fn read_message(&self, stream: &mut TcpStream) -> io::Result<(u32, Option<MessageId>, Vec<u8>)> {
        let mut buf_len = [0u8; 4];

        if let Err(e) = stream.read_exact(&mut buf_len).await {
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
                return Ok((0, None, vec![]));
            }
            return Err(self.debug_err("Connection lost during length read:", e));
        }
        let len = u32::from_be_bytes(buf_len);
        if len == 0 {
            return Ok((0, None, vec![]));
        }

        let mut buf_id = [0u8; 1];
        if let Err(e) = stream.read_exact(&mut buf_id).await {
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
                return Ok((0, None, vec![]));
            }
            return Err(self.debug_err("Connection lost during id read:", e));
        }
        let id = MessageId::try_from(buf_id[0])?;
        if len == 1 {
            return Ok((len, Some(id), vec![]));
        }

        let mut buf_payload = vec![0u8; (len - 1) as usize];
        if let Err(e) = stream.read_exact(&mut buf_payload).await {
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
                return Ok((0, None, vec![]));
            }
            return Err(self.debug_err("Connection lost during payload read:", e));
        }

        Ok((len, Some(id), buf_payload))
    }

    async fn send_interested(&self, stream: &mut TcpStream) -> io::Result<()>{
        let mut msg = Vec::with_capacity(5);
        msg.extend_from_slice(&1u32.to_be_bytes());
        msg.push(2);
        stream.write_all(msg.as_slice()).await?;

        self.debug("Wrote messages len 1, id 2");
        Ok(())
    }

    async fn request_piece(&self, stream: &mut TcpStream, len: u32, id: u8, index: u32, begin: u32, length: u32) -> io::Result<()>{
        let mut msg = Vec::with_capacity(17);
        msg.extend_from_slice(&len.to_be_bytes());
        msg.push(id);
        msg.extend_from_slice(&index.to_be_bytes());
        msg.extend_from_slice(&begin.to_be_bytes());
        msg.extend_from_slice(&length.to_be_bytes());
        stream.write_all(msg.as_slice()).await?;

        self.debug(format!("Sent message len {len}, id {id}, index {index}, begin: {begin}, length: {length}").as_str());
        Ok(())
    }

    fn calc_block_size(piece_length: u32, requested: u32) -> u32 {
        if piece_length - requested >=  Self::BLOCK_SIZE {
            Self::BLOCK_SIZE
        } else {
            piece_length - requested
        }
    }

    pub fn verify_piece(&self, index: usize, data: &[u8]) -> bool {
        let start = index * 20;
        let end = (index + 1) * 20;
        let expected = &self.torrent.pieces[start..end];

        let mut hasher = Sha1::new();
        hasher.update(data);
        let actual = hasher.finalize();

        expected == actual.as_slice()
    }

    pub fn claim_piece(&self, peer_session: &PeerSession) -> Option<usize> {
        let mut state = self.state.lock().unwrap();

        for i in 0..self.torrent.count_pieces() as usize {
            if !state.is_piece_complete(i) && !state.is_piece_in_progress(i) && peer_session.has_piece(i) {
                state.set_in_progress(i, true);
                return Some(i);
            }
        }
        None
    }

    pub async fn download(self: Arc<Self>) -> io::Result<()> {
        let peers = self.torrent.get_peers(self.peer_id.as_str()).await?;
        let semaphore = Arc::new(tokio::sync::Semaphore::new(20));

        let mut set = tokio::task::JoinSet::new();
        for peer in peers {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let addr = format!("{}:{}", peer.0, peer.1);
            let d = Arc::clone(&self);

            set.spawn(async move {
                let _permit = permit;
                let _ = Self::run_peer_session(d, addr).await;
            });
        }

        loop {
            if self.state.lock().unwrap().is_complete(self.torrent.count_pieces() as usize) {
                set.abort_all();
                break;
            }

            if set.is_empty() {
                break;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        Ok(())
    }

    fn write_piece(&self, piece: u32, bytes: &[u8]) -> io::Result<()> {
        if self.verify_piece(piece as usize, &bytes) {
            let mut file = self.file.lock().unwrap();

            let offset = (piece as u64) * (self.torrent.piece_length as u64);
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            Ok(())
        } else {
            Err(io::Error::new(ErrorKind::Other, format!("SHA1 check failed for piece {}", piece)))
        }

    }

    pub async fn run_peer_session(d: Arc<Download>, addr: String) -> io::Result<()> {
        let mut stream = timeout(Duration::from_secs(5), TcpStream::connect(&addr)).await??;

        match d.torrent.send_handshake(&mut stream, d.peer_id.as_str()).await {
            Ok(_) => d.debug(&format!("Handshake successful with {}", addr)),
            Err(e) => {
                d.debug("Handshake failed, switching to another peer");
                return Err(e)
            }
        }

        d.send_interested(&mut stream).await?;

        let mut peer_session = PeerSession { bitfield: vec![], choked: true };

        let mut buf = vec![];
        let mut received = 0u32;
        let mut requested = 0u32;
        let mut piece_index = None;
        let mut piece_length = 0;
        let total_pieces = d.torrent.count_pieces() as usize;

        loop {
            if d.state.lock().unwrap().is_complete(total_pieces) {
                return Ok(());
            }
            
            d.debug(&format!("Waiting for message... (Piece {:?}, Received {})", piece_index, received));
            if piece_index.is_none() && !peer_session.choked {
                if let Some(piece) = d.claim_piece(&peer_session) {
                    piece_index = Some(piece as u32);
                    piece_length = d.torrent.calc_piece_length(piece as u32);
                    buf = vec![0u8; piece_length as usize];
                    received = 0u32;
                    requested = 0u32;
                }
            }

            if piece_index.is_some() && !peer_session.choked {
                if let Some(piece) = piece_index {
                    loop {
                        if (requested - received) / Self::BLOCK_SIZE >= Self::MAX_REQUESTS {
                            break;
                        }
                        let block_size = Self::calc_block_size(piece_length, requested);
                        if block_size > 0 {
                            d.request_piece(&mut stream, 13, 6, piece, requested, block_size).await?;
                            requested += block_size;
                        } else {
                            break;
                        }
                    }
                }
            }

            let message = timeout(Duration::from_secs(120), d.read_message(&mut stream)).await?;

            match message {
                Ok((_, Some(id), payload)) => {
                    match id {
                        MessageId::Choke => {
                            peer_session.choked = true;
                            // cancel work in progress
                            if let Some(piece) = piece_index {
                                let mut state = d.state.lock().unwrap();
                                state.set_in_progress(piece as usize, false);
                                piece_index = None;
                            }
                            d.debug(&format!("Received message {:?}", id));
                        },
                        MessageId::Unchoke => {
                            peer_session.choked = false;
                            d.debug(&format!("Received message {:?}", id));
                        },
                        MessageId::Have => {
                            peer_session.update_have(u32::from_be_bytes(payload[..4].try_into().unwrap()) as usize);
                            d.debug(&format!("Received message {:?}", id));
                        },
                        MessageId::Bitfield => {
                            peer_session.bitfield = payload;
                            d.debug(&format!("Received message {:?}", id));
                        },
                        MessageId::Piece => {
                            if let Some(piece) = piece_index {
                                let begin = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                                let data = &payload[8..];
                                received += data.len() as u32;
                                buf[begin as usize..begin as usize + data.len()].copy_from_slice(data);
                                if received >= piece_length {
                                    let mut state = d.state.lock().unwrap();
                                    match d.write_piece(piece, &buf) {
                                        Ok(_) => {
                                           state.set_complete(piece as usize);
                                           peer_session.update_have(piece as usize);
                                           state.progress_bar.inc(buf.len() as u64);
                                           state.progress_bar.set_message(format!("Piece {}/{}", state.count_complete(), d.torrent.count_pieces()));
                                           piece_index = None;
                                        },
                                        Err(e) => {
                                            state.set_in_progress(piece as usize, false);
                                            piece_index = None;
                                            d.debug(&e.to_string());
                                        }
                                    }
                                }
                            } else {
                                d.debug("Received Piece message, but I've got no piece!")
                            }
                        },
                        id => {
                            d.debug(&format!("Received message {:?}", id));
                        },
                    };
                },
                Ok(_) => (),
                Err(e) => {
                    d.debug(&format!("got error: {}", &e));
                    if let Some(piece) = piece_index {
                        let mut state = d.state.lock().unwrap();
                        state.set_in_progress(piece as usize, false);
                    }
                    return Err(e)
                }
            }
        }
    }

}