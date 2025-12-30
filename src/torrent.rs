use sha1::{Digest, Sha1, Sha1Core};
use sha1::digest::core_api::CoreWrapper;
use sha1::digest::Output;
use crate::bencode::{decode_bencoded_value, BencodeNode, BencodeValue};

pub struct Torrent<'a> {
    pub url: String,
    pub length: isize,
    pub piece_length: isize,
    pub pieces: &'a [u8],
    pub info_hash: Output<CoreWrapper<Sha1Core>>,
}

impl<'a> Torrent<'a> {
    pub fn new(data: &'a [u8]) -> Torrent<'a> {
        let (node, _) = decode_bencoded_value(data);
        let d = match &node.value {
            BencodeValue::Dict(d) => d,
            _ => panic!("Expected dictionary")
        };

        let url = match d.get("announce") {
            Some(BencodeNode { value: BencodeValue::String(url), raw: _ }) => url.clone(),
            _ => panic!("Torrent URL not found"),
        };

        let node_info = d.get("info").expect("Expected info");
        let mut hasher = Sha1::new();
        hasher.update(node.raw);
        let info_hash = hasher.finalize();

        let info_dict = match &node_info.value {
            BencodeValue::Dict(d) => d,
            _ => panic!("Expected dictionary")
        };

        let length = match info_dict.get("length") {
            Some(BencodeNode { value: BencodeValue::Integer(len), raw: _ }) => *len,
            _ => panic!("Torrent URL not found"),
        };

        let piece_length = match info_dict.get("piece length") {
            Some(BencodeNode { value: BencodeValue::Integer(len), raw: _ }) => *len,
            _ => panic!("Torrent URL not found"),
        };

        let pieces = match info_dict.get("pieces") {
            Some(BencodeNode { value: BencodeValue::Binary(_), raw }) => *raw,
            _ => panic!("Torrent URL not found"),
        };

        Torrent { url, length, piece_length, pieces, info_hash }
    }
}