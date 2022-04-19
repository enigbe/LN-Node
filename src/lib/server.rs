use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use bitcoin::network::constants::Network;

// Node variables passed to application state
#[derive(Debug, Clone)]
pub struct NodeVariable {
	pub network: Network,
}

/// Get help information on how to query the node
async fn help(req: HttpRequest) -> HttpResponse {
	// 1. Get path, i.e. "help"
	let path = req.path().to_owned();
	// 2. Get additional data from request body if presented/required
	// 3. Send data into an server-dedicated channel on the node that is waiting for input
	// 4. Get response from node and send back to CLI client

	HttpResponse::Ok().finish()
}

/// Get node information
async fn nodeinfo(req: HttpRequest, node_var: web::Data<NodeVariable>) -> HttpResponse {
	println!("node network: {:?}", req);
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

pub async fn run(node_var: NodeVariable) -> Result<Server, std::io::Error> {
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
