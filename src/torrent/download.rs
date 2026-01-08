use crate::torrent::torrent;
use sha1::{Digest, Sha1};
use std::fs::File;
use std::io;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

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
        if byte_index > self.bitfield.len() {
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

pub struct Download<'a> {
    torrent: &'a torrent::Torrent<'a>,
    peer_id: &'a str,
    output_dir: String,
    output_file: String,
    debug: bool,
}

impl<'a> Download<'a> {
    pub fn new(torrent: &'a torrent::Torrent<'a>, output_dir: String, output_file: String, debug: bool) -> Download<'a> {
        Download { torrent, peer_id: "avknevkjn43t34tn389f", output_dir, output_file, debug }
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

    fn read_message(&self, stream: &mut TcpStream) -> io::Result<(u32, Option<MessageId>, Vec<u8>)> {
        let mut buf_len = [0u8; 4];

        if let Err(e) = stream.read_exact(&mut buf_len) {
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
        if let Err(e) = stream.read_exact(&mut buf_id) {
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
        if let Err(e) = stream.read_exact(&mut buf_payload) {
            if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
                return Ok((0, None, vec![]));
            }
            return Err(self.debug_err("Connection lost during payload read:", e));
        }

        Ok((len, Some(id), buf_payload))
    }

    fn send_interested(&self, stream: &mut TcpStream) -> io::Result<()>{
        let mut msg = Vec::with_capacity(5);
        msg.extend_from_slice(&1u32.to_be_bytes());
        msg.push(2);
        stream.write_all(msg.as_slice())?;

        self.debug("Wrote messages len 1, id 2");
        Ok(())
    }

    fn request_piece(&self, stream: &mut TcpStream, len: u32, id: u8, index: u32, begin: u32, length: u32) -> io::Result<()>{
        let mut msg = Vec::with_capacity(17);
        msg.extend_from_slice(&len.to_be_bytes());
        msg.push(id);
        msg.extend_from_slice(&index.to_be_bytes());
        msg.extend_from_slice(&begin.to_be_bytes());
        msg.extend_from_slice(&length.to_be_bytes());
        stream.write_all(msg.as_slice())?;

        self.debug(format!("Sent message len {len}, id {id}, index {index}, begin: {begin}, length: {length}").as_str());
        Ok(())
    }

    fn calc_block_size(piece_length: u32, received: u32) -> u32 {
        if piece_length - received >= 16 * 1024 {
            16 * 1024
        } else {
            piece_length - received
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

    fn download_piece(&self, stream: &mut TcpStream, piece: u32, peer_session: &mut PeerSession) -> io::Result<Vec<u8>> {
        let piece_length = self.torrent.calc_piece_length(piece);

        let mut buf = vec![0u8; piece_length as usize];
        let mut received = 0u32;

        if !peer_session.choked {
            let block_size = Self::calc_block_size(piece_length, received);
            self.request_piece(stream, 13, 6, piece, 0, block_size)?;
        }

        loop {
            self.debug(format!("Waiting for message... (Piece {}, Received {})", piece, received).as_str());
            match self.read_message(stream) {
                Ok((len, Some(id), payload)) => {
                    self.debug(format!("Read message len {len}, id {:?}, payload size {}", id, payload.len()).as_str());
                    match id {
                        MessageId::Choke => {
                            peer_session.choked = true;
                            return Err(io::Error::new(io::ErrorKind::Interrupted, "Choked"));
                        }
                        MessageId::Unchoke => {
                            peer_session.choked = false;
                            let block_size = Self::calc_block_size(piece_length, received);
                            self.request_piece(stream, 13, 6, piece, 0, block_size)?;
                        },
                        MessageId::Have => {
                            peer_session.update_have(u32::from_be_bytes(payload[..4].try_into().unwrap()) as usize);
                        }
                        MessageId::Piece => {
                            let begin = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                            let data = &payload[8..];
                            received += data.len() as u32;
                            buf[begin as usize..begin as usize + data.len()].copy_from_slice(data);
                            if received >= piece_length {
                                return Ok(buf);
                            }
                            let block_size = Self::calc_block_size(piece_length, received);
                            self.request_piece(stream, 13, 6, piece, begin + data.len() as u32, block_size)?;
                        },
                        id => eprintln!("Received unhandled message id {:?}", id),
                    };
                },
                Ok(_) => (),
                Err(e) => {
                    eprintln!("got error: {}", e);
                    break;
                }
            }
        }
        Ok(vec![])
    }

    pub fn download(&self) -> io::Result<()> {
        let start_time = Instant::now();
        let final_path = format!("{}/{}", &self.output_dir, &self.output_file);
        let mut output_file = File::create(&final_path)?;
        output_file.set_len(self.torrent.length as u64)?;

        let peers = self.torrent.get_peers(self.peer_id)?;
        let pieces = self.torrent.count_pieces();
        let mut completed = vec![false; pieces as usize];

        for peer in &peers {
            self.debug(format!("Connecting to peer {}", peer.0).as_str());
            if let Ok(mut stream) = TcpStream::connect_timeout(&SocketAddr::new(IpAddr::from(peer.0), peer.1), Duration::from_secs(5)) {
                stream.set_read_timeout(Some(Duration::from_secs(10)))?;
                stream.set_write_timeout(Some(Duration::from_secs(10)))?;
                let mut peer_session = PeerSession { bitfield: vec![], choked: true };
                match self.torrent.send_handshake(&mut stream, self.peer_id) {
                    Ok(_) => self.debug("Peer accepted handshake"),
                    Err(_) => {
                        self.debug("Handshake failed, switching to another peer");
                        continue
                    }
                }
                loop {
                    match self.read_message(&mut stream) {
                        Ok((_, Some(id), payload)) => match id {
                            MessageId::Bitfield => peer_session.bitfield = payload,
                            MessageId::Unchoke => peer_session.choked = false,
                            MessageId::Have => (),
                            _ => (),
                        },
                        _ => break,
                    }
                }
                self.send_interested(&mut stream)?;
                for piece_index in 0..pieces {
                    if completed[piece_index as usize] || !peer_session.has_piece(piece_index as usize) {
                        continue;
                    }
                    match self.download_piece(&mut stream, piece_index, &mut peer_session) {
                        Ok(bytes) => {
                            if self.verify_piece(piece_index as usize, &bytes) {
                                let offset = (piece_index as u64) * (self.torrent.piece_length as u64);
                                output_file.seek(SeekFrom::Start(offset))?;
                                output_file.write_all(&bytes)?;

                                completed[piece_index as usize] = true;
                                self.print_progress(&completed, start_time);
                            } else {
                                self.debug(format!("SHA1 check failed for piece {}", piece_index).as_str());
                            }
                            continue
                        },
                        Err(_) => continue,
                   }
                }

            }
            if completed.iter().all(|x| *x) {
                break;
            }
        }

        if completed.iter().all(|x| *x) {
            self.debug("\nDownload finished");
        } else {
            self.debug(format!("\nDownload finished with {} pieces missing", completed.iter().filter(|x| **x == false).count()).as_str());
        }
        Ok(())
    }

    fn print_progress(&self, completed: &[bool], start_time: Instant) {
        let pieces_count = completed.len();
        let completed_count = completed.iter().filter(|x| **x).count();

        let bytes_done = completed.iter().enumerate()
            .filter(|(_, done)| **done)
            .map(|(piece, _)| self.torrent.calc_piece_length(piece as u32))
            .sum::<u32>();
        let bytes_total = self.torrent.length as u64;
        let elapsed = start_time.elapsed().as_secs_f64();

        let speed = if elapsed > 0.0 {
            (bytes_done as f64 / (1024.0 * 1024.0)) / elapsed
        } else {
            0.0
        };

        let eta = if speed > 0.0 {
            let remaining_mb = (bytes_total - bytes_done as u64) as f64 / (1024.0 * 1024.0);
            remaining_mb / speed
        } else {
            0.0
        };

        let percent = (bytes_done as f64 / bytes_total as f64) * 100.0;
        let bar_width = 25;
        let filled = ((percent / 100.0) * bar_width as f64) as usize;
        let empty = bar_width - filled;

        print!(
            "\r[{}{}] {:>6.2}% | {:>7.2} MB/s | ETA: {:>3.0}s | {}/{} pieces",
            "=".repeat(filled),
            " ".repeat(empty),
            percent,
            speed,
            eta,
            completed_count,
            pieces_count
        );
        let _ = io::stdout().flush();
    }
}