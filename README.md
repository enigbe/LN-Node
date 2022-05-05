# LN-Node

Re-architecting the LDK's sample node implementation to have a separate CLI binary and actix-web server. LN-Node's is a lightning node with a separate CLI binary that should work akin to
`LND` and its command line interface tool `lncli`

## Installation

```bash
$ git clone https://github.com/enigbe/LN-Node
$ cd LN-Node
```

## Usage

1. Start the LDK node

```
$ ./lnnode.sh
```

2. Switch to another terminal and run commands with the CLI

```bash
$ cargo run --bin lnnode-cli help
```

3. You can also test with a REST client. I have attached a JSON file of the API environment containing all endpoints. Import the `insomnia_rest_api.json` file into your Insomnia

## License

Licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
