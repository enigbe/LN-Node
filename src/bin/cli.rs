use lnnode::server::{Help, NodeInfo, ServerSuccess};
use reqwest;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

/// LDK CLI command
#[derive(Debug, Serialize)]
struct Command();

impl Command {
	/// Create a new variation of a Command
	///
	/// # Arguments
	/// * cmd_input (&Vec<String>): a vector of Strings representing the terminal environment
	///   variables
	fn new(cmd_input: &Vec<String>) -> HashMap<String, String> {
		let arg = cmd_input[1].trim().to_lowercase();
		match arg.as_str() {
			"openchannel" => {
				// TODO: parse optional `public` parameter for channel_announcement
				let channel_info_parts: Vec<&str> = cmd_input[2].split("@").collect();
				let host_info_parts: Vec<&str> = channel_info_parts[1].split(":").collect();
				let pub_key = channel_info_parts[0].to_string();
				let host = host_info_parts[0].to_string();
				let port = host_info_parts[1].to_string();
				let channel_amt_satoshis = cmd_input[3].clone();

				let mut map = HashMap::new();
				map.insert("pubkey".to_string(), pub_key);
				map.insert("host".to_string(), host);
				map.insert("port".to_string(), port);
				map.insert("channel_amt_satoshis".to_string(), channel_amt_satoshis);

				return map;
			}
			"sendpayment" => {
				let invoice = cmd_input[2].to_string();

				let mut map = HashMap::new();
				map.insert("invoice".to_string(), invoice);

				return map;
			}
			"getinvoice" => {
				let amt_millisatoshis = cmd_input[2].to_string();

				let mut map = HashMap::new();
				map.insert("amt_millisatoshis".to_string(), amt_millisatoshis);

				return map;
			}
			"connectpeer" => {
				if cmd_input.len() < 3 {
					println!("LN-Node peer connection information:");
					println!("-----------------------------------");
					println!("\tError: invalid connectpeer command");
					println!("\tProvide peer connection details in format: <pubkey@host:port>");
					let map = HashMap::new();

					return map;
				}

				let channel_info_parts: Vec<&str> = cmd_input[2].split("@").collect();
				let host_info_parts: Vec<&str> = channel_info_parts[1].split(":").collect();
				let pub_key = channel_info_parts[0].to_string();
				let host = host_info_parts[0].to_string();
				let port = host_info_parts[1].to_string();

				let mut map = HashMap::new();

				map.insert("pubkey".to_string(), pub_key);
				map.insert("host".to_string(), host);
				map.insert("port".to_string(), port);

				return map;
			}
			"listchannels" => {
				let map = HashMap::new();
				return map;
			}
			"listpayments" => {
				let map = HashMap::new();
				return map;
			}
			"closechannel" => {
				let channel_id = cmd_input[2].to_string();

				let mut map = HashMap::new();
				map.insert("channel_id".to_string(), channel_id);

				return map;
			}
			"forceclosechannel" => {
				let channel_id = cmd_input[2].to_string();
				let mut map = HashMap::new();
				map.insert("channel_id".to_string(), channel_id);

				return map;
			}
			"nodeinfo" => {
				let map = HashMap::new();
				return map;
			}
			"listpeers" => {
				let map = HashMap::new();
				return map;
			}
			"signmessage" => {
				let message = cmd_input[2].to_string();

				let mut map = HashMap::new();
				map.insert("message".to_string(), message);

				return map;
			}
			"help" => {
				let map = HashMap::new();
				return map;
			}
			_ => {
				let map = HashMap::new();
				return map;
			}
		}
	}
}

#[tokio::main]
async fn main() {
	let valid_commands: Vec<&str> = vec![
		"help",
		"nodeinfo",
		"connectpeer",
		"listpeers",
		"openchannel",
		"listchannels",
		"getinvoice",
		"sendpayment",
		"listpayments",
		"closechannel",
		"forceclosechannel",
		"signmessage",
	];
	// 1. Get argument list/vector from terminal
	let cmd_args: Vec<String> = env::args().collect();
	if cmd_args.len() < 2 {
		println!(
			"You must provide an argument to the lnnode-cli command, e.g. lnnode-cli nodeinfo"
		);
		return;
	}

	// 2. Parse to appropriate command
	let command = Command::new(&cmd_args);
	let mut count: u8 = 0;
	for cmd in valid_commands {
		if cmd_args[1].to_lowercase().as_str() == cmd {
			count = count + 1;
			continue;
		}
	}
	if count == 0 {
		println!("Entered an invalid command. Enter `lnnode-cli help` to get list of all commands");
		return;
	}
	// 3. Create a request body with matching map

	let cli_client = reqwest::Client::new();
	let port: u32 = 33335;
	let node_server_url = format!("http://127.0.0.1:{}", port);
	let path = cmd_args[1].clone();
	let url = format!("{}/{}", node_server_url.as_str(), path);

	// let req_body = serde_json::to_string(&command).unwrap();

	// 4. Send request to node server
	let resp = cli_client.post(url).json(&command).send().await.unwrap();

	// 5. Match the response to designed enum types and process accordingly
	match path.as_str() {
		"help" => {
			let help_resp = resp.json::<Help>().await;
			match help_resp {
				Ok(help) => {
					println!("LN-Node help commands:");
					println!("-----------------------------------");
					println!("\tnodeinfo: {:?}", help.nodeinfo);
					println!("\topenchannel: {:?}", help.openchannel);
					println!("\tsendpayment: {:?}", help.sendpayment);
					println!("\tgetinvoice: {:?}", help.getinvoice);
					println!("\tconnectpeer: {:?}", help.connectpeer);
					println!("\tlistchannels: {:?}", help.listchannels);
					println!("\tlistpeers: {:?}", help.listpeers);
					println!("\tclosechannel: {:?}", help.closechannel);
					println!("\tforceclosechannel: {:?}", help.forceclosechannel);
					println!("\tlistpayments: {:?}", help.listpayments);
					println!("\tsignmessage: {:?}", help.signmessage);
				}
				Err(e) => {
					println!("LN-Node-server error: {}", e);
				}
			}
		}
		"nodeinfo" => {
			let nodeinfo_resp = resp.json::<NodeInfo>().await;
			match nodeinfo_resp {
				Ok(info) => {
					println!("LN-Node node information:");
					println!("-----------------------------------");
					println!("\tpubkey: {:?}", info.pubkey);
					println!("\tchannels_number: {:?}", info.channels_number);
					println!("\tusable_channels_number: {:?}", info.usable_channels_number);
					println!("\tlocal_balance_msat: {:?}", info.local_balance_msat);
					println!("\tpeers: {:?}", info.peers);
				}
				Err(e) => {
					println!("LN-Node-server error: {}", e);
				}
			}
		}
		"connectpeer" => {
			let connectpeer_resp = resp.json::<ServerSuccess>().await;
			match connectpeer_resp {
				Ok(peer_msg) => {
					println!("LN-Node peer connection information:");
					println!("-----------------------------------");
					println!("\tconnection message: {:?}", peer_msg.msg);
				}
				Err(e) => {
					println!("LN-Node-server error: {}", e);
				}
			}
		}
		"openchannel" => {
			let openchannel_resp = resp.json::<ServerSuccess>().await;
			match openchannel_resp {
				Ok(openchannel_msg) => {
					println!("LN-Node opening a payment channel:");
					println!("-----------------------------------");
					println!("\tchannel message: {:?}", openchannel_msg.msg);
				}
				Err(e) => {
					println!("LN-Node-server error: {}", e);
				}
			}
		}
		"listpeers" => {}
		"listchannels" => {}
		"getinvoice" => {}
		"sendpayment" => {}
		"closechannel" => {}
		"forceclosechannel" => {}
		"signmessage" => {}
		_ => (),
	}
	// 6. Write server response to terminal
}
