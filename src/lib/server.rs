use crate::bitcoind_client::BitcoindClient;
use crate::cli::{connect_peer_if_necessary, parse_peer_info, sanitize_string};
use crate::handle_ldk_events;
use crate::hex_utils;
use crate::node_var::{
	ChannelManager, HTLCStatus, InvoicePayer, MillisatAmount, PaymentInfo, PaymentInfoStorage,
	PeerManager,
};
use actix_web::dev::Server;
use actix_web::{http::header::ContentType, web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::hashes::Hash;
use bitcoin::network::constants::Network;
use bitcoin::secp256k1::PublicKey;
use lightning::chain::keysinterface::KeysManager;
use lightning::ln::PaymentHash;
use lightning::routing::network_graph::NetworkGraph;
use lightning::routing::network_graph::NodeId;
use lightning::util::config::ChannelConfig;
use lightning::util::config::ChannelHandshakeLimits;
use lightning::util::config::UserConfig;
use lightning::util::events::{Event, EventHandler};
use lightning_invoice::{utils, Currency};
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
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
	pub tokio_handle: tokio::runtime::Handle,
	pub channel_manager: Arc<ChannelManager>,
	pub bitcoind_client: Arc<BitcoindClient>,
	pub keys_manager: Arc<KeysManager>,
	pub inbound_payments: PaymentInfoStorage,
	pub outbound_payments: PaymentInfoStorage,
	pub network: Network,
}

impl EventHandler for ServerEventHandler {
	fn handle_event(&self, event: &Event) {
		self.tokio_handle.block_on(handle_ldk_events(
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
	pub openchannel: String,
	pub sendpayment: String,
	pub getinvoice: String,
	pub connectpeer: String,
	pub listchannels: String,
	pub listpayments: String,
	pub closechannel: String,
	pub forceclosechannel: String,
	pub nodeinfo: String,
	pub listpeers: String,
	pub signmessage: String,
}

// Struct containing the list of peers a node has
#[derive(Serialize, Deserialize, Debug)]
pub struct ListPeers {
	pub peers: Vec<PublicKey>,
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

// openchannel request struct
#[derive(Serialize, Deserialize, Debug)]
pub struct OpenChannel {
	pubkey: PublicKey,
	host: String,
	port: String,
	amt_satoshis: String,
}

// connectpeer struct
#[derive(Serialize, Deserialize, Debug)]
pub struct ConnectPeer {
	pubkey: PublicKey,
	host: String,
	port: String,
}

// getinvoice struct
#[derive(Serialize, Deserialize, Debug)]
pub struct GetInvoice {
	amt_millisatoshis: String,
}

// invoice/payment request struct
#[derive(Serialize, Deserialize, Debug)]
pub struct Invoice {
	payment_hash: String,
	preimage: String,
	secret: String,
	status: HTLCStatus,
	amt_msat: MillisatAmount,
}

// Server Error
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerError {
	error: String,
}

// Server suceess
#[derive(Serialize, Deserialize, Debug)]
pub struct ServerSuccess {
	msg: String,
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

/// Open channel with another node
async fn open_channel(
	req: web::Json<OpenChannel>, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	let pubkey = &req.pubkey;
	let host = &req.host;
	let port = &req.port;
	let amt_satoshis = &req.amt_satoshis.parse::<u64>().unwrap();
	let announced_channel = true;
	let channel_manager = &node_var.channel_manager;

	let config = UserConfig {
		peer_channel_config_limits: ChannelHandshakeLimits {
			their_to_self_delay: 2016,
			..Default::default()
		},
		channel_options: ChannelConfig { announced_channel, ..Default::default() },
		..Default::default()
	};

	match channel_manager.create_channel(*pubkey, *amt_satoshis, 0, 0, Some(config)) {
		Ok(_) => {
			let msg =
				ServerSuccess { msg: format!("Channel successfully opened with: {}", pubkey) };
			println!("EVENT: initiated channel with peer {}. ", pubkey);
			return HttpResponse::Ok().content_type(ContentType::json()).json(msg);
		}
		Err(e) => {
			let error = ServerError { error: format!("{:?}", e) };
			return HttpResponse::ExpectationFailed().content_type(ContentType::json()).json(error);
		}
	}
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

/// Connect to another peer
async fn connect_peer(
	req: web::Json<ConnectPeer>, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	let peer_manager = node_var.peer_manager.clone();
	let pubkey = format!("{}", req.pubkey);
	let host = format!("{}", req.host);
	let port = format!("{}", req.port);
	let peer_pubkey_host_port = format!("{}@{}:{}", pubkey, host, port);

	if pubkey == "" || host == "" || port == "" {
		let error = ServerError {
			error:
				"ERROR: connectpeer requires peer connection info: `connectpeer pubkey@host:port`"
					.to_string(),
		};
		return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
	} else {
		let pubkey_peer_addr = parse_peer_info(peer_pubkey_host_port);
		match pubkey_peer_addr {
			Ok(info) => {
				if connect_peer_if_necessary(info.0, info.1, peer_manager).await.is_ok() {
					let msg =
						ServerSuccess { msg: format!("SUCCESS: connected to peer {}", info.0) };
					return HttpResponse::Ok().content_type(ContentType::json()).json(msg);
				} else {
					let error = ServerError { error: "Failed to connect to peer".to_string() };
					return HttpResponse::BadRequest()
						.content_type(ContentType::json())
						.json(error);
				}
			}
			Err(e) => {
				let error = ServerError { error: e.into_inner().unwrap().to_string() };
				return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
			}
		};
	}
}

/// Get invoice
async fn get_invoice(
	req: web::Json<GetInvoice>, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	let amt_str = format!("{}", req.amt_millisatoshis);
	if amt_str == "" {
		let error = ServerError {
			error: "ERROR: getinvoice requires an amount in millisatoshis".to_string(),
		};
		return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
	}

	let amt_msat: Result<u64, _> = amt_str.parse();
	if amt_msat.is_err() {
		let error = ServerError {
			error: "ERROR: getinvoice provided payment amount was not a number".to_string(),
		};
		return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
	}

	let inbound_payments = node_var.inbound_payments.clone();
	let channel_manager = node_var.channel_manager.clone();
	let keys_manager = node_var.keys_manager.clone();
	let network = node_var.network;

	let mut payments = inbound_payments.lock().unwrap();
	let currency = match network {
		Network::Bitcoin => Currency::Bitcoin,
		Network::Testnet => Currency::BitcoinTestnet,
		Network::Regtest => Currency::Regtest,
		Network::Signet => Currency::Signet,
	};

	let amt_msat = amt_msat.unwrap();
	let invoice = utils::create_invoice_from_channelmanager(
		&channel_manager,
		keys_manager,
		currency,
		Some(amt_msat),
		"ln-node".to_string(),
	);

	match invoice {
		Ok(inv) => {
			let payment_hash = PaymentHash(inv.payment_hash().clone().into_inner());
			payments.insert(
				payment_hash,
				PaymentInfo {
					preimage: None,
					secret: Some(inv.payment_secret().clone()),
					status: HTLCStatus::Pending,
					amt_msat: MillisatAmount(Some(amt_msat)),
				},
			);

			let x: &[_] = &['\\', '"'];
			let payment_hash_string =
				hex_utils::hex_str(&payment_hash.0).trim_matches(x).to_string();
			let payment_request = Invoice {
				payment_hash: format!("{:?}", payment_hash_string),
				preimage: "".to_string(),
				secret: format!("{:?}", Some(inv.payment_secret().clone())),
				status: HTLCStatus::Pending,
				amt_msat: MillisatAmount(Some(amt_msat)),
			};
			return HttpResponse::Ok().content_type(ContentType::json()).json(payment_request);
		}
		Err(e) => {
			let error = ServerError { error: format!("ERROR: failed to create invoice: {:?}", e) };
			return HttpResponse::Ok().content_type(ContentType::json()).json(error);
		}
	}
}

/// Send payment
async fn send_payment(
	req: web::Json<GetInvoice>, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	todo!()
}

/// Run the server
pub fn run(node_var: NodeVar<ServerEventHandler>, addr: &str) -> Result<Server, std::io::Error> {
	let node_var = web::Data::new(node_var);
	let listener = TcpListener::bind(addr).expect("Failed to bind on random port");
	let port = listener.local_addr().unwrap().port();

	println!("Server port: {}", port);

	let server = HttpServer::new(move || {
		App::new()
			.route("/nodeinfo", web::post().to(nodeinfo))
			.route("/connectpeer", web::post().to(connect_peer))
			.route("/openchannel", web::post().to(open_channel))
			.route("/help", web::post().to(help))
			.route("/listchannels", web::post().to(list_channels))
			.route("/listpeers", web::post().to(list_peers))
			.route("/getinvoice", web::post().to(get_invoice))
			.route("/sendpayment", web::post().to(send_payment))
			.app_data(node_var.clone())
	})
	.listen(listener)?
	.run();

	Ok(server)
}
