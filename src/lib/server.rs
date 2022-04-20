use std::sync::Arc;

use crate::node_var::{
	ChannelManager, HTLCStatus, InvoicePayer, MillisatAmount, PaymentInfo, PaymentInfoStorage,
	PeerManager,
};

use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::network::constants::Network;
use lightning::chain::keysinterface::KeysManager;
use lightning::routing::network_graph::NetworkGraph;
use lightning::util::events::{Event, EventHandler};

// Node variables passed to application state
#[derive(Clone)]
pub struct NodeVariable<E>
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
async fn nodeinfo<E: EventHandler>(
	req: HttpRequest, node_var: web::Data<NodeVariable<E>>,
) -> HttpResponse {
	println!("request: {:?}", req);
	println!("node network: {:?}", node_var.network);
	HttpResponse::Ok().body("Success")
}

/// List node peers
async fn list_peers() -> HttpResponse {
	HttpResponse::Ok().finish()
}

/// Connect another node to running node
async fn connect_peer() -> HttpResponse {
	HttpResponse::Ok().finish()
}

pub fn run<E: EventHandler + Send + Sync>(
	node_var: NodeVariable<E>,
) -> Result<Server, std::io::Error> {
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
