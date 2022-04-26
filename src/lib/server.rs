use crate::bitcoind_client::BitcoindClient;
use crate::cli::sanitize_string;
use crate::handle_ldk_events;
use crate::hex_utils;
use crate::node_var::{ChannelManager, InvoicePayer, PaymentInfoStorage, PeerManager};
use actix_web::dev::Server;
use actix_web::{http::header::ContentType, web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::hash_types::Txid;
use bitcoin::network::constants::Network;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::keysinterface::KeysManager;
use lightning::ln::channelmanager::ChannelDetails;
use lightning::ln::msgs::ChannelAnnouncement;
use lightning::routing::network_graph::NetworkGraph;
use lightning::routing::network_graph::NodeId;
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
	// channel_list: Vec<ChannelDetails>,
	channels_number: usize,
	usable_channels_number: usize,
	local_balance_msat: u64,
	peers: usize,
}

// Help command struct
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

// Struct containing the list of peers a node has
#[derive(Serialize, Deserialize, Debug)]
pub struct ListPeers {
	peers: Vec<PublicKey>,
}

// Struct containing redefined channel details
#[derive(Serialize, Deserialize, Debug)]
pub struct RedefinedChannelDetails {
	channel_id: String,
	tx_id: String,
	peer_pubkey: String,
	peer_alias: String,
	short_channel_id: u64,
	is_confirmed_onchain: bool,
	local_balance_msat: u64,
	channel_value_satoshis: u64,
	available_balance_for_send_msat: u64,
	available_balance_for_recv_msat: u64,
	channel_can_send_payments: bool,
	public: bool,
}

// Struct containing the list of channels a node has
#[derive(Serialize, Deserialize, Debug)]
pub struct ListChannels {
	channels: Vec<RedefinedChannelDetails>,
}

/// Get helpful information on how to interact with the lightning node
async fn help(_req: HttpRequest) -> HttpResponse {
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
		// channel_list,
		channels_number,
		usable_channels_number,
		local_balance_msat,
		peers,
	};

	HttpResponse::Ok().content_type(ContentType::json()).json(nodeinfo_obj)
	// HttpResponse::Ok().finish()
}

/// List connected node peers
async fn list_peers(node_var: web::Data<NodeVar<ServerEventHandler>>) -> HttpResponse {
	let peers = node_var.peer_manager.get_peer_node_ids();
	if peers.len() == 0 {
		let list_peers = ListPeers { peers: Vec::new() };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_peers);
	} else {
		let list_peers = ListPeers { peers };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_peers);
	}
}

///List open node channels
async fn list_channels(node_var: web::Data<NodeVar<ServerEventHandler>>) -> HttpResponse {
	let channel_manager = &node_var.channel_manager;
	let network_graph = &node_var.network_graph;
	let channels_list = channel_manager.list_channels();
	let mut channel_vector = Vec::new();

	if channels_list.len() == 0 {
		let list_channels = ListChannels { channels: Vec::new() };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_channels);
	} else {
		for chan_info in channels_list {
			let chan_id = hex_utils::hex_str(&chan_info.channel_id[..]);

			let mut txid = String::new();
			if let Some(funding_txo) = chan_info.funding_txo {
				txid = format!("{}", funding_txo.txid);
			}
			let peer_pubkey = hex_utils::hex_str(&chan_info.counterparty.node_id.serialize());

			let mut peer_alias = String::new();
			if let Some(node_info) = network_graph
				.read_only()
				.nodes()
				.get(&NodeId::from_pubkey(&chan_info.counterparty.node_id))
			{
				if let Some(announcement) = &node_info.announcement_info {
					peer_alias = sanitize_string(&announcement.alias);
				}
			}

			let mut short_channel_id: u64 = 0;
			if let Some(id) = chan_info.short_channel_id {
				short_channel_id = id;
			}

			let is_confirmed_onchain = chan_info.is_funding_locked;
			let channel_value_satoshis = chan_info.channel_value_satoshis;
			let local_balance_msat = chan_info.balance_msat;

			let mut available_balance_for_send_msat = 0;
			let mut available_balance_for_recv_msat = 0;
			if chan_info.is_usable {
				available_balance_for_send_msat = chan_info.outbound_capacity_msat;
				available_balance_for_recv_msat = chan_info.inbound_capacity_msat;
			}

			let channel_can_send_payments = chan_info.is_usable;
			let public = chan_info.is_public;

			// Create RedefinedChannelDetails and add to vector
			let chan_details = RedefinedChannelDetails {
				channel_id: chan_id,
				tx_id: txid,
				peer_pubkey,
				peer_alias,
				short_channel_id,
				is_confirmed_onchain,
				local_balance_msat,
				channel_value_satoshis,
				available_balance_for_send_msat,
				available_balance_for_recv_msat,
				channel_can_send_payments,
				public,
			};

			channel_vector.push(chan_details);
		}
		let list_channels = ListChannels { channels: channel_vector };
		return HttpResponse::Ok().content_type(ContentType::json()).json(list_channels);
	}
}

pub fn run(node_var: NodeVar<ServerEventHandler>) -> Result<Server, std::io::Error> {
	let node_var = web::Data::new(node_var);
	let server = HttpServer::new(move || {
		App::new()
			.route("/nodeinfo", web::post().to(nodeinfo))
			.route("/help", web::post().to(help))
			.route("/listchannels", web::post().to(list_channels))
			.route("/listpeers", web::post().to(list_peers))
			.app_data(node_var.clone())
	})
	.bind("127.0.0.1:8080")?
	.run();

	Ok(server)
}
