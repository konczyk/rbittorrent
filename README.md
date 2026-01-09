# RBittorrent

A minimal BitTorrent client for downloading .torrent files via the command line.

## Features
- Parses .torrent files (bencode decoding, info hash calculation).
- Supports HTTP and UDP trackers to fetch peers.
- Connects to peers, performs handshake, and manages choke/unchoke messages.
- Downloads pieces with SHA1 verification for data integrity.
- Shows real-time progress: completion %, download speed, ETA, and piece count.

## Notes
- Only supports single-file torrents.
- Default peer port is 6881.

## Testing
```shell
cargo test 
```

## Running
```shell
cargo run
```

Options
```shell
cargo run -- -h
```

## Examples

Display torrent info
```shell
$ cargo run -- info data/debian.torrent
Tracker URL: http://bttracker.debian.org:6969/announce
Length: 822083584
Info Hash: b2387d1a5eb488b8b60ed1eebec698fa20dfac34
Piece Length: 262144
Piece Hashes:
e5d037b6007e42490df1e6949c461bc9dd13c715
c27059fc817c8354d30e559a2ebd6f55aace5a15
af7142ba25677a57d777dcf75b16ba74c26467a2
287391747a176df9ce3f83a79b3a0d6fcf915103
b7ab11d25a5e3fa8809f5ef2398b3871a2158f19
[...]
```

Download torrent
```shell
$ cargo run -- -o /tmp download data/debian.torrent
⠠ [00:01:51] [>---------------------------------------] 18.25 MiB/784.00 MiB (66m) Piece 75/3136
```
