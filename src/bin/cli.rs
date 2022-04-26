use reqwest;
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;

/// LDK CLI command
#[derive(Debug, Serialize)]
enum Command {
	OpenChannel { pub_key: String, host: String, port: String, amount_satoshis: String },
	SendPayment { invoice: String },
	GetInvoice { amt_millisatoshis: String },
	ConnectPeer { pub_key: String, host: String, port: String },
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
				let channel_info_parts: Vec<&str> = cmd_input[2].split("@").collect();
				let host_info_parts: Vec<&str> = channel_info_parts[1].split(":").collect();
				let pub_key = channel_info_parts[0].to_string();
				let host = host_info_parts[0].to_string();
				let port = host_info_parts[1].to_string();
				let amount_satoshis = cmd_input[2].clone();
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
				let channel_info_parts: Vec<&str> = cmd_input[2].split("@").collect();
				let host_info_parts: Vec<&str> = channel_info_parts[1].split(":").collect();
				let pub_key = channel_info_parts[0].to_string();
				let host = host_info_parts[0].to_string();
				let port = host_info_parts[1].to_string();
				return Some(Command::ConnectPeer { pub_key, host, port });
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

#[derive(Debug, Deserialize)]
enum ServerResp {
	Help,
	ListPeers,
	ListChannels,
}

impl ServerResp {
	fn process(resp: Result<Self, reqwest::Error>) -> () {
		todo!()
	}
}

#[derive(Debug, Serialize)]
struct PostBody {
	command: Command,
}

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

	println!("{:?}", command);
	// 3. Create a request body with command
	// 3.1 Construct a client and request body {"command": command}
	let cli_client = reqwest::Client::new();
	let node_server_url = "http://127.0.0.1:8080";
	let path = cmd_args[1].clone();
	let url = format!("{}/{}", node_server_url, path);
	let req_body = serde_json::to_string(&command).unwrap();

	// 4. Send request to node server
	let resp = cli_client.post(url).body(req_body).send().await.unwrap().json::<ServerResp>().await;

	println!("{:?}", resp);

	// 5. Match the response to designed enum types and process accordingly
	// 6. Write server response to terminal
}
