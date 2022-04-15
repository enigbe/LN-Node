use actix_web::dev::Server;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use lnnode::start_ldk;

/// Get help information on how to query the node
async fn help(req: HttpRequest) -> HttpResponse {
	// 1. Get path, i.e. "help"
	let path = req.path().to_owned();
	// 2. Find a way to pass this into the node's CLI
	start_ldk().await;

	HttpResponse::Ok().finish()
}

/// Get node information
async fn nodeinfo() -> HttpResponse {
	HttpResponse::Ok().finish()
}

/// List node peers
async fn list_peers() -> HttpResponse {
	HttpResponse::Ok().finish()
}

/// Connect another node to running node
async fn connect_peer() -> HttpResponse {
	HttpResponse::Ok().finish()
}

fn run() -> Result<Server, std::io::Error> {
	let server = HttpServer::new(|| {
		App::new()
			.route("/nodeinfo", web::get().to(nodeinfo))
			.route("/help", web::get().to(help))
			.route("/connect_peer", web::get().to(connect_peer))
			.route("/list_peers", web::get().to(list_peers))
	})
	.bind("127.0.0.1:8000")?
	.run();

	Ok(server)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
	println!("Starting node server on port: 8000");
	run()?.await
}
