use crate::bitcoind_client::BitcoindClient;
use crate::handle_ldk_events;
use crate::node_var::{ChannelManager, InvoicePayer, PaymentInfoStorage, PeerManager};
use actix_web::dev::Server;
use actix_web::{http::header::ContentType, web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::network::constants::Network;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::keysinterface::KeysManager;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::msgs::ChannelAnnouncement;
use lightning::routing::network_graph::NetworkGraph;
use lightning::util::events::{Event, EventHandler};
use serde::{Deserialize, Serialize};
use std::string::String;
use std::sync::Arc;

// Node variables passed to application state
#[derive(Clone)]
pub struct NodeVar<E>
where
	E: EventHandler,
{
	pub invoice_payer: Arc<InvoicePayer<E>>,
	pub peer_manager: Arc<PeerManager>,
	pub channel_manager: Arc<ChannelManager>,
	pub keys_manager: Arc<KeysManager>,
	pub network_graph: Arc<NetworkGraph>,
	pub network: Network,
	pub inbound_payments: PaymentInfoStorage,
	pub outbound_payments: PaymentInfoStorage,
	pub ldk_data_dir: String,
}

pub struct ServerEventHandler {
	pub channel_manager: Arc<ChannelManager>,
	pub bitcoind_client: Arc<BitcoindClient>,
	pub keys_manager: Arc<KeysManager>,
	pub inbound_payments: PaymentInfoStorage,
	pub outbound_payments: PaymentInfoStorage,
	pub network: Network,
}

impl EventHandler for ServerEventHandler {
	fn handle_event(&self, event: &Event) {
		let handle = tokio::runtime::Handle::current();
		handle.block_on(handle_ldk_events(
			self.channel_manager.clone(),
			self.bitcoind_client.clone(),
			self.keys_manager.clone(),
			self.inbound_payments.clone(),
			self.outbound_payments.clone(),
			self.network,
			event,
		));
	}
}

// NodeInfo struct
#[derive(Serialize, Deserialize, Debug)]
pub struct NodeInfo {
	pubkey: PublicKey,
	channel_list: Vec<ChannelDetails>,
	channels_number: usize,
	usable_channels_number: usize,
	local_balance_msat: u64,
	peers: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Help {
	openchannel: String,
	sendpayment: String,
	getinvoice: String,
	connectpeer: String,
	listchannels: String,
	listpayments: String,
	closechannel: String,
	forceclosechannel: String,
	nodeinfo: String,
	listpeers: String,
	signmessage: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListPeers {
	peers: Vec<PublicKey>,
}

/// Get helpful information on how to interact with the lightning node
async fn help(_req: HttpRequest) -> HttpResponse {
	// 1. Return a JSON body with key-value pairs where the keys are the commands
	//	  and the bodies are the command parameters
	// 1.1 Construct HelpCommand instance
	let help = Help {
		openchannel: "pubkey@host:port <amt_satoshis>".to_string(),
		sendpayment: "<invoice>".to_string(),
		getinvoice: "<amt_millisatoshis>".to_string(),
		connectpeer: "pubkey@host:port".to_string(),
		listchannels: "".to_string(),
		listpayments: "".to_string(),
		closechannel: "<channel_id>".to_string(),
		forceclosechannel: "<channel_id>".to_string(),
		nodeinfo: "".to_string(),
		listpeers: "".to_string(),
		signmessage: "<message>".to_string(),
	};
	// 1.2 Return response
	HttpResponse::Ok().content_type(ContentType::json()).json(help)
}

/// Get node information
async fn nodeinfo(
	req: HttpRequest, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	let pubkey = node_var.channel_manager.get_our_node_id();
	let channel_list = node_var.channel_manager.list_channels();
	let channels_number = channel_list.len();
	let usable_channels_number = channel_list.iter().filter(|c| c.is_usable).count();
	let local_balance_msat = channel_list.iter().map(|c| c.balance_msat).sum::<u64>();
	let peers = node_var.peer_manager.get_peer_node_ids().len();

	// Construct response body and return response
	let nodeinfo_obj = NodeInfo {
		pubkey,
		channel_list,
		channels_number,
		usable_channels_number,
		local_balance_msat,
		peers,
	};

	HttpResponse::Ok().content_type(ContentType::json()).json(nodeinfo_obj)
	// HttpResponse::Ok().finish()
}

/// List node peers
async fn list_peers(node_var: web::Data<NodeVar<ServerEventHandler>>) -> HttpResponse {
	let peers = node_var.peer_manager.get_peer_node_ids();
	if peers.len() == 0 {
		// 1. Return empty list of peers
		let list_peers = ListPeers { peers: Vec::new() };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_peers);
	} else {
		let list_peers = ListPeers { peers };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_peers);
	}
}

/// Connect another node to running node
async fn connect_peer() -> HttpResponse {
	HttpResponse::Ok().finish()
}

pub fn run(node_var: NodeVar<ServerEventHandler>) -> Result<Server, std::io::Error> {
	let node_var = web::Data::new(node_var);
	let server = HttpServer::new(move || {
		App::new()
			.route("/nodeinfo", web::post().to(nodeinfo))
			.route("/help", web::post().to(help))
			.route("/connect_peer", web::post().to(connect_peer))
			.route("/list_peers", web::post().to(list_peers))
			.app_data(node_var.clone())
	})
	.bind("127.0.0.1:8080")?
	.run();

	Ok(server)
}
