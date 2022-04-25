use crate::bitcoind_client::BitcoindClient;
use crate::hex_utils;
use crate::node_var::{
	ChannelManager, HTLCStatus, InvoicePayer, MillisatAmount, PaymentInfo, PaymentInfoStorage,
	PeerManager,
};
use actix_web::dev::Server;
use actix_web::{http::header::ContentType, web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::network::constants::Network;
use futures::executor::block_on;
use std::io;
// use bitcoin::secp256k1::PublicKey;
use crate::handle_ldk_events;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::consensus::encode;
use bitcoin::secp256k1::Secp256k1;
use bitcoin_bech32::WitnessProgram;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning::chain::keysinterface::KeysManager;
use lightning::routing::network_graph::NetworkGraph;
use lightning::util::events::{Event, EventHandler, PaymentPurpose};
use rand::{thread_rng, Rng};
use serde::Serialize;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

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
#[derive(Serialize)]
pub struct NodeInfo {
	// pubkey: String,
	// channel_list: Vec<ChannelDetails>,
	channels_number: usize,
	usable_channels_number: usize,
	local_balance_msat: u64,
	peers: usize,
}

/// Get help information on how to query the node
async fn help(req: HttpRequest) -> HttpResponse {
	// 1. Get path, i.e. "help"
	let path = req.path().to_owned();
	// 2. Get additional data from request body if presented/required
	// 3. Send data into an server-dedicated channel on the node that is waiting for input
	// 4. Get response from node and send back to CLI client
	println!("node network: {:?}", path);
	HttpResponse::Ok().finish()
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
		// pubkey,
		channels_number,
		usable_channels_number,
		local_balance_msat,
		peers,
	};

	HttpResponse::Ok().content_type(ContentType::json()).json(nodeinfo_obj)
}

/// List node peers
async fn list_peers() -> HttpResponse {
	HttpResponse::Ok().finish()
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
