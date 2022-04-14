use actix_web::dev::Server;
use actix_web::{web, App, HttpResponse, HttpServer};

/// Start the LDK sample node
async fn start() -> HttpResponse {
	HttpResponse::Ok().finish()
}

fn run() -> Result<Server, std::io::Error> {
	let server = HttpServer::new(|| App::new().route("/", web::get().to(start)))
		.bind("127.0.0.1:8000")?
		.run();

	Ok(server)
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
	run()?.await
}
