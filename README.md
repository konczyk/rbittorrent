# RBittorrent

Simple bittorrent client 

## Usage

Build project
```shell
cargo build -r
```

Run tests
```shell
cargo test
```

Execute
```shell
$ ./target/release/rbittorrent -h
Usage: rbittorrent <COMMAND> <VALUE>

Arguments:
  <COMMAND>  Name of the command [possible values: decode, info]
  <VALUE>    Value to process

Options:
  -h, --help  Print help
```

## Examples

Decode bencoded value
```shell
$ ./target/release/rbittorrent decode d4:key1li78ei-873ee4:key25:valuee
{"key1":[78,-873],"key2":"value"}
```

Display torrent info
```shell
$ ./target/release/rbittorrent info data/ubuntu.torrent
Tracker URL: https://torrent.ubuntu.com/announce
Length: 5702520832
Info Hash: c8295ce630f2064f08440db1534e4992cfe4862a
Piece Length: 262144
Piece Hashes:
555bb58fab9093efced0b48e0058d03ee91d1770
397be5eb6241f1d7a35196af264737881742b090
c4d19aaf32910a792cbd3e4c4b165e531423f77b
c3ce98dc99225b07f535710d6b8c5058c387ae4e
7de04dbf50b1361fa3a11d93e3e130ec37381695
ed422cd04ac921b7730a22e84fe74fb34cf58d0f
[...]
```

Discover peers
```shell
$ ./target/release/rbittorrent peers data/ubuntu.torrent
185.125.190.59:6881
```