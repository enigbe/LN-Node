use crate::hex_utils;
use std::io;
use crate::bitcoind_client::BitcoindClient;
use crate::node_var::{
	ChannelManager, HTLCStatus, InvoicePayer, MillisatAmount, PaymentInfo, PaymentInfoStorage,
	PeerManager,
};
use actix_web::dev::Server;
use actix_web::{ web, App, HttpRequest, HttpResponse, HttpServer, http::header::ContentType};
use bitcoin::network::constants::Network;
// use bitcoin::secp256k1::PublicKey;
use bitcoin_bech32::WitnessProgram;
use lightning::chain::keysinterface::KeysManager;
use lightning::routing::network_graph::NetworkGraph;
use lightning::util::events::{Event, EventHandler, PaymentPurpose};
use std::collections::HashMap;
use std::sync::Arc;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::consensus::encode;
use std::collections::hash_map::Entry;
use rand::{thread_rng, Rng};
use std::time::{Duration};
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use bitcoin::secp256k1::Secp256k1;
use std::io::Write;
use serde::Serialize;

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
	// tokio_handle: tokio::runtime::Handle,
	pub channel_manager: Arc<ChannelManager>,
	pub bitcoind_client: Arc<BitcoindClient>,
	pub keys_manager: Arc<KeysManager>,
	pub inbound_payments: PaymentInfoStorage,
	pub outbound_payments: PaymentInfoStorage,
	pub network: Network,
}

impl EventHandler for ServerEventHandler {
	fn handle_event(&self, event: &Event) {
		tokio::runtime::Handle::current().spawn(async move {
			match event {
				Event::FundingGenerationReady {
					temporary_channel_id,
					channel_value_satoshis,
					output_script,
					..
				} => {
					// Construct the raw transaction with one output, that is paid the amount of the
					// channel.
					let addr = WitnessProgram::from_scriptpubkey(
						&output_script[..],
						match self.network {
							Network::Bitcoin => bitcoin_bech32::constants::Network::Bitcoin,
							Network::Testnet => bitcoin_bech32::constants::Network::Testnet,
							Network::Regtest => bitcoin_bech32::constants::Network::Regtest,
							Network::Signet => bitcoin_bech32::constants::Network::Signet,
						},
					)
					.expect("Lightning funding tx should always be to a SegWit output")
					.to_address();
					let mut outputs = vec![HashMap::with_capacity(1)];
					outputs[0].insert(addr, *channel_value_satoshis as f64 / 100_000_000.0);
					let raw_tx = self.bitcoind_client.create_raw_transaction(outputs).await;

					// Have your wallet put the inputs into the transaction such that the output is
					// satisfied.
					let funded_tx = self.bitcoind_client.fund_raw_transaction(raw_tx).await;

					// Sign the final funding transaction and broadcast it.
					let signed_tx = self.bitcoind_client.sign_raw_transaction_with_wallet(funded_tx.hex).await;
					assert_eq!(signed_tx.complete, true);
					let final_tx: Transaction =
						encode::deserialize(&hex_utils::to_vec(&signed_tx.hex).unwrap()).unwrap();
					// Give the funding transaction back to LDK for opening the channel.
					if self.channel_manager
						.funding_transaction_generated(&temporary_channel_id, final_tx)
						.is_err()
					{
						println!(
							"\nERROR: Channel went away before we could fund it. The peer disconnected or refused the channel.");
						print!("> ");
						io::stdout().flush().unwrap();
					}
				},
				Event::PaymentReceived { payment_hash, purpose, amt, .. } => {
					let mut payments = self.inbound_payments.lock().unwrap();
					let (payment_preimage, payment_secret) = match purpose {
						PaymentPurpose::InvoicePayment { payment_preimage, payment_secret, .. } => {
							(*payment_preimage, Some(*payment_secret))
						}
						PaymentPurpose::SpontaneousPayment(preimage) => (Some(*preimage), None),
					};
					let status = match self.channel_manager.claim_funds(payment_preimage.unwrap()) {
						true => {
							println!(
								"\nEVENT: received payment from payment hash {} of {} millisatoshis",
								hex_utils::hex_str(&payment_hash.0),
								amt
							);
							print!("> ");
							io::stdout().flush().unwrap();
							HTLCStatus::Succeeded
						}
						_ => HTLCStatus::Failed,
					};
					match payments.entry(*payment_hash) {
						Entry::Occupied(mut e) => {
							let payment = e.get_mut();
							payment.status = status;
							payment.preimage = payment_preimage;
							payment.secret = payment_secret;
						}
						Entry::Vacant(e) => {
							e.insert(PaymentInfo {
								preimage: payment_preimage,
								secret: payment_secret,
								status,
								amt_msat: MillisatAmount(Some(*amt)),
							});
						}
					}
				},
				Event::PaymentSent { payment_preimage, payment_hash, fee_paid_msat, .. } => {
					let mut payments = self.outbound_payments.lock().unwrap();
					for (hash, payment) in payments.iter_mut() {
						if *hash == *payment_hash {
							payment.preimage = Some(*payment_preimage);
							payment.status = HTLCStatus::Succeeded;
							println!(
								"\nEVENT: successfully sent payment of {} millisatoshis{} from \
										payment hash {:?} with preimage {:?}",
								payment.amt_msat,
								if let Some(fee) = fee_paid_msat {
									format!(" (fee {} msat)", fee)
								} else {
									"".to_string()
								},
								hex_utils::hex_str(&payment_hash.0),
								hex_utils::hex_str(&payment_preimage.0)
							);
							print!("> ");
							io::stdout().flush().unwrap();
						}
					}
				},
				Event::OpenChannelRequest { .. } => {
					// Unreachable, we don't set manually_accept_inbound_channels
				},
				Event::PaymentPathSuccessful { .. } => {},
				Event::PaymentPathFailed { .. } => {},
				Event::PaymentFailed { payment_hash, .. } => {
					print!(
						"\nEVENT: Failed to send payment to payment hash {:?}: exhausted payment retry attempts",
						hex_utils::hex_str(&payment_hash.0)
					);
					print!("> ");
					io::stdout().flush().unwrap();
		
					let mut payments = self.outbound_payments.lock().unwrap();
					if payments.contains_key(&payment_hash) {
						let payment = payments.get_mut(&payment_hash).unwrap();
						payment.status = HTLCStatus::Failed;
					}
				},
				Event::PaymentForwarded { fee_earned_msat, claim_from_onchain_tx } => {
					let from_onchain_str = if *claim_from_onchain_tx {
						"from onchain downstream claim"
					} else {
						"from HTLC fulfill message"
					};
					if let Some(fee_earned) = fee_earned_msat {
						println!(
							"\nEVENT: Forwarded payment, earning {} msat {}",
							fee_earned, from_onchain_str
						);
					} else {
						println!("\nEVENT: Forwarded payment, claiming onchain {}", from_onchain_str);
					}
					print!("> ");
					io::stdout().flush().unwrap();
				},
				Event::PendingHTLCsForwardable { time_forwardable } => {
					let forwarding_channel_manager = self.channel_manager.clone();
					let min = time_forwardable.as_millis() as u64;
					tokio::spawn(async move {
						let millis_to_sleep = thread_rng().gen_range(min, min * 5) as u64;
						tokio::time::sleep(Duration::from_millis(millis_to_sleep)).await;
						forwarding_channel_manager.process_pending_htlc_forwards();
					});
				},
				Event::SpendableOutputs { outputs } => {
					let destination_address = self.bitcoind_client.get_new_address().await;
					let output_descriptors = &outputs.iter().map(|a| a).collect::<Vec<_>>();
					let tx_feerate =
						self.bitcoind_client.get_est_sat_per_1000_weight(ConfirmationTarget::Normal);
					let spending_tx = self.keys_manager
						.spend_spendable_outputs(
							output_descriptors,
							Vec::new(),
							destination_address.script_pubkey(),
							tx_feerate,
							&Secp256k1::new(),
						)
						.unwrap();
					self.bitcoind_client.broadcast_transaction(&spending_tx);
				},
				Event::ChannelClosed { channel_id, reason, user_channel_id: _ } => {
					println!(
						"\nEVENT: Channel {} closed due to: {:?}",
						hex_utils::hex_str(channel_id),
						reason
					);
					print!("> ");
					io::stdout().flush().unwrap();
				},
				Event::DiscardFunding { .. } => {
					// A "real" node should probably "lock" the UTXOs spent in funding transactions until
					// the funding transaction either confirms, or this event is generated.
				},
			}
			// todo!()
		});			
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
