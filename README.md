# RBittorrent

Simple bittorrent client 

## Usage

Build
```shell
$ cargo build -r
```

Test
```shell
$ cargo test
```

Run
```shell
$ ./target/release/rbittorrent -h
Usage: rbittorrent [OPTIONS] <COMMAND> <TORRENT_FILE>

Arguments:
  <COMMAND>       Command name [possible values: info, download]
  <TORRENT_FILE>  A torrent file

Options:
  -d, --debug                    Run in debug mode
  -o, --output-dir <OUTPUT_DIR>  Directory to save the file into
  -h, --help                     Print help
```

## Examples

Display torrent info
```shell
$ ./target/release/rbittorrent info data/debian.torrent
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
$ ./target/release/rbittorrent -o /tmp download data/debian.torrent
[                         ]   3.41% |    0.04 MB/s | ETA: 17848s | 107/3136 pieces
```
