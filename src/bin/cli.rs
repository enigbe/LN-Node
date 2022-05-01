use lnnode::server::{Help, NodeInfo, ServerSuccess};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{collections::HashMap, env};

/// LDK CLI command
#[derive(Debug, Serialize)]
enum Command {
	OpenChannel { pub_key: String, host: String, port: String, amount_satoshis: String },
	SendPayment { invoice: String },
	GetInvoice { amt_millisatoshis: String },
	ConnectPeer { map: HashMap<String, String> },
	ListChannels,
	ListPayments,
	CloseChannel { channel_id: String },
	ForceCloseChannel { channel_id: String },
	NodeInfo,
	ListPeers,
	SignMessage { message: String },
	Help,
}

impl Command {
	/// Create a new variation of a Command
	///
	/// # Arguments
	/// * cmd_input (&Vec<String>): a vector of Strings representing the terminal environment
	///   variables
	fn new(cmd_input: &Vec<String>) -> Option<Self> {
		let arg = cmd_input[1].trim().to_lowercase();
		match arg.as_str() {
			"openchannel" => {
				// TODO: parse optional `public` parameter
				let channel_info_parts: Vec<&str> = cmd_input[2].split("@").collect();
				let host_info_parts: Vec<&str> = channel_info_parts[1].split(":").collect();
				let pub_key = channel_info_parts[0].to_string();
				let host = host_info_parts[0].to_string();
				let port = host_info_parts[1].to_string();
				let amount_satoshis = cmd_input[3].clone();
				return Some(Command::OpenChannel { pub_key, host, port, amount_satoshis });
			}
			"sendpayment" => {
				let invoice = cmd_input[2].to_string();
				return Some(Command::SendPayment { invoice });
			}
			"getinvoice" => {
				let amt_millisatoshis = cmd_input[2].to_string();
				return Some(Command::GetInvoice { amt_millisatoshis });
			}
			"connectpeer" => {
				if cmd_input.len() < 3 {
					println!("LN-Node peer connection information:");
					println!("-----------------------------------");
					println!("\tError: invalid connectpeer command");
					println!("\tProvide peer connection details in format: <pubkey@host:port>");
					return None;
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

				return Some(Command::ConnectPeer { map });
			}
			"listchannels" => Some(Command::ListChannels),
			"listpayments" => Some(Command::ListPayments),
			"closechannel" => {
				let channel_id = cmd_input[2].to_string();
				return Some(Command::CloseChannel { channel_id });
			}
			"forceclosechannel" => {
				let channel_id = cmd_input[2].to_string();
				return Some(Command::ForceCloseChannel { channel_id });
			}
			"nodeinfo" => Some(Command::NodeInfo),
			"listpeers" => Some(Command::ListPeers),
			"signmessage" => {
				let message = cmd_input[2].to_string();
				return Some(Command::SignMessage { message });
			}
			"help" => {
				return Some(Command::Help);
			}
			_ => None,
		}
	}
}

// #[derive(Debug, Serialize)]
// struct PostBody {
// 	command: Command,
// }

#[tokio::main]
async fn main() {
	// 1. Get argument list/vector from terminal
	let cmd_args: Vec<String> = env::args().collect();
	if cmd_args.len() < 2 {
		println!(
			"You must provide an argument to the lnnode-cli command, e.g. lnnode-cli nodeinfo"
		);
		return;
	}
	// 2. Parse to appropriate Command enum
	let command = Command::new(&cmd_args);
	match command {
		None => {}
		Some(comm) => {
			// 3. Create a request body with command
			// 3.1 Construct a client and request body {"command": command}
			let cli_client = reqwest::Client::new();
			let port: u32 = 33335;
			let node_server_url = format!("http://127.0.0.1:{}", port);
			let path = cmd_args[1].clone();
			let url = format!("{}/{}", node_server_url.as_str(), path);
			let req_body = serde_json::to_string(&comm).unwrap();

			// 4. Send request to node server
			// let resp = cli_client.post(url).body(req_body).send().await.unwrap().json::<ServerResp>().await;
			let resp = cli_client.post(url).body(req_body).send().await.unwrap();

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
				"openchannel" => {}
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
	}
}
