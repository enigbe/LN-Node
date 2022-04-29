use crate::bitcoind_client::BitcoindClient;
use crate::cli;
use crate::cli::{connect_peer_if_necessary, parse_peer_info, sanitize_string};
use crate::hex_utils;
use crate::node_var::{
	ChannelManager, HTLCStatus, InvoicePayer, MillisatAmount, PaymentInfo, PaymentInfoStorage,
	PeerManager,
};
use crate::{disk, handle_ldk_events};
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
use lightning_invoice::payment::PaymentError;
use lightning_invoice::{utils, Currency, Invoice};
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::ops::Deref;
use std::path::Path;
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
	pubkey: String,
	host: String,
	port: String,
	channel_amt_satoshis: String,
	channel_announcement: Option<String>,
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
pub struct ServerInvoice {
	invoice: String,
}

// payment struct
#[derive(Serialize, Deserialize, Debug)]
pub struct Payment {
	amount_millisatoshis: String,
	payment_hash: String,
	htlc_direction: String,
	htlc_status: String,
}

// payments struct
#[derive(Serialize, Deserialize, Debug)]
pub struct Payments {
	payments: Vec<Payment>,
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
	let pubkey = req.pubkey.clone();
	let host = req.host.clone();
	let port = req.port.clone();
	let channel_amt_satoshis = req.channel_amt_satoshis.clone();
	let channel_announcement = req.channel_announcement.clone();
	let peer_manager = node_var.peer_manager.clone();

	// Validate critical (required) user arguments
	if pubkey == "".to_string()
		|| host == "".to_string()
		|| port == "".to_string()
		|| channel_amt_satoshis == "".to_string()
	{
		let error = ServerError {
			error: format!("ERROR: openchannel has 2 required arguments: `openchannel pubkey@host:port channel_amt_satoshis` [--public]").to_string(),
		};
		return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
	}

	// Get public key and socket address from supplied parameters
	let peer_pubkey_and_ip_addr = format!("{}@{}:{}", pubkey, host, port);
	let pubkey_peeraddr = parse_peer_info(peer_pubkey_and_ip_addr.to_string());

	match pubkey_peeraddr {
		Ok(info) => {
			let chan_amt_sat: Result<u64, _> = channel_amt_satoshis.parse();
			if chan_amt_sat.is_err() {
				let error =
					ServerError { error: format!("ERROR: channel amount must be a number") };
				return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
			}

			if connect_peer_if_necessary(info.0, info.1, peer_manager.clone()).await.is_err() {
				let error = ServerError { error: format!("ERROR: cannot connect to peer") };
				return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
			};

			let announce_channel = match channel_announcement {
				Some(val) => {
					if val.as_str() == "true" {
						true
					} else {
						false
					}
				}
				None => false,
			};

			if cli::open_channel(
				info.0,
				chan_amt_sat.unwrap(),
				announce_channel,
				node_var.channel_manager.clone(),
			)
			.is_ok()
			{
				let peer_data_path = format!("{}/channel_peer_data", node_var.ldk_data_dir.clone());
				let _ = disk::persist_channel_peer(
					Path::new(&peer_data_path),
					peer_pubkey_and_ip_addr.as_str(),
				);

				let msg = ServerSuccess {
					msg: format!("EVENT: initiated channel with peer {}. ", info.0),
				};
				return HttpResponse::Ok().content_type(ContentType::json()).json(msg);
			} else {
				let error =
					ServerError { error: format!("ERROR: unable to open a channel with peer") };
				return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
			}
		}
		Err(e) => {
			let error = ServerError { error: format!("{:?}", e.into_inner().unwrap()) };
			return HttpResponse::BadRequest().content_type(ContentType::json()).json(error);
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

	if pubkey == "".to_string() || host == "".to_string() || port == "".to_string() {
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

			let inv_str = ServerInvoice { invoice: format!("{}", inv) };
			return HttpResponse::Ok().content_type(ContentType::json()).json(inv_str);
		}
		Err(e) => {
			let error = ServerError { error: format!("ERROR: failed to create invoice: {:?}", e) };
			return HttpResponse::Ok().content_type(ContentType::json()).json(error);
		}
	}
}

/// Send payment
async fn send_payment(
	req: web::Json<ServerInvoice>, node_var: web::Data<NodeVar<ServerEventHandler>>,
) -> HttpResponse {
	let invoice = req.invoice.parse::<Invoice>().unwrap();
	let invoice_payer = node_var.invoice_payer.clone();
	let payment_storage = node_var.outbound_payments.clone();

	let payment_id = invoice_payer.pay_invoice(&invoice);
	match payment_id {
		Ok(_payment_id) => {
			let payee_pubkey = invoice.recover_payee_pub_key();
			let amt_msat = invoice.amount_milli_satoshis().unwrap();

			let status = HTLCStatus::Pending;

			let payment_hash = PaymentHash(invoice.payment_hash().clone().into_inner());
			let payment_secret = Some(invoice.payment_secret().clone());

			let mut payments = payment_storage.lock().unwrap();
			payments.insert(
				payment_hash,
				PaymentInfo {
					preimage: None,
					secret: payment_secret,
					status,
					amt_msat: MillisatAmount(invoice.amount_milli_satoshis()),
				},
			);
			let payment_msg = ServerSuccess {
				msg: format!("EVENT: initiated sending {} msats to {}", amt_msat, payee_pubkey),
			};
			return HttpResponse::Ok().content_type(ContentType::json()).json(payment_msg);
		}
		Err(PaymentError::Invoice(e)) => {
			let error = ServerError { error: format!("ERROR: invalid invoice: {}", e) };
			return HttpResponse::ExpectationFailed().content_type(ContentType::json()).json(error);
		}
		Err(PaymentError::Routing(e)) => {
			let error = ServerError { error: format!("ERROR: failed to find route: {}", e.err) };
			return HttpResponse::ExpectationFailed().content_type(ContentType::json()).json(error);
		}
		Err(PaymentError::Sending(e)) => {
			let error = ServerError { error: format!("ERROR: failed to send payment: {:?}", e) };
			return HttpResponse::ExpectationFailed().content_type(ContentType::json()).json(error);
		}
	}
}

/// List payments
async fn list_payments(node_var: web::Data<NodeVar<ServerEventHandler>>) -> HttpResponse {
	let inbound = node_var.inbound_payments.lock().unwrap();
	let outbound = node_var.outbound_payments.lock().unwrap();

	// 1. create payments vector
	let mut payments_vec: Vec<Payment> = Vec::new();
	// 2. loop through inbound and outbound payments and append payments to vec
	for (payment_hash, payment_info) in inbound.deref() {
		let payment = Payment {
			amount_millisatoshis: format!("{}", payment_info.amt_msat),
			payment_hash: hex_utils::hex_str(&payment_hash.0),
			htlc_direction: "inbound".to_string(),
			htlc_status: match payment_info.status {
				HTLCStatus::Pending => "pending".to_string(),
				HTLCStatus::Succeeded => "succeeded".to_string(),
				HTLCStatus::Failed => "failed".to_string(),
			},
		};
		payments_vec.push(payment);
	}

	for (payment_hash, payment_info) in outbound.deref() {
		let payment = Payment {
			amount_millisatoshis: format!("{}", payment_info.amt_msat),
			payment_hash: hex_utils::hex_str(&payment_hash.0),
			htlc_direction: "outbound".to_string(),
			htlc_status: match payment_info.status {
				HTLCStatus::Pending => "pending".to_string(),
				HTLCStatus::Succeeded => "succeeded".to_string(),
				HTLCStatus::Failed => "failed".to_string(),
			},
		};
		payments_vec.push(payment);
	}
	let payments = Payments { payments: payments_vec };
	return HttpResponse::Ok().content_type(ContentType::json()).json(payments);
}

/// Run the server
pub fn run(node_var: NodeVar<ServerEventHandler>, addr: &str) -> Result<Server, std::io::Error> {
	let node_var = web::Data::new(node_var);
	// let listener = TcpListener::bind(addr).expect("Failed to bind on random port");
	// let port = listener.local_addr().unwrap().port();

	println!("Server port: {}", addr);

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
			.route("/listpayments", web::post().to(list_payments))
			.app_data(node_var.clone())
	})
	.bind(addr)?
	.run();

	Ok(server)
}
