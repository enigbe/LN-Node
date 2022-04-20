use crate::bitcoind_client::BitcoindClient;
use crate::disk::FilesystemLogger;
use lightning::chain;
use lightning::chain::chainmonitor;
use lightning::chain::keysinterface::{InMemorySigner, KeysInterface, KeysManager, Recipient};
use lightning::chain::Filter;
use lightning::ln::channelmanager::{
	ChainParameters, ChannelManagerReadArgs, SimpleArcChannelManager,
};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, SimpleArcPeerManager};
use lightning::ln::{PaymentHash, PaymentPreimage, PaymentSecret};
use lightning::routing::network_graph::{NetGraphMsgHandler, NetworkGraph};
use lightning::routing::scoring::ProbabilisticScorer;
use lightning_background_processor::{BackgroundProcessor, Persister};
use lightning_invoice::payment;
use lightning_invoice::utils::DefaultRouter;
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::FilesystemPersister;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

/// Defines the status variations of an HTLC
pub enum HTLCStatus {
	Pending,
	Succeeded,
	Failed,
}

pub struct MillisatAmount(pub Option<u64>);

impl fmt::Display for MillisatAmount {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self.0 {
			Some(amt) => write!(f, "{}", amt),
			None => write!(f, "unknown"),
		}
	}
}

/// Defines the information about a payment
pub struct PaymentInfo {
	pub preimage: Option<PaymentPreimage>,
	pub secret: Option<PaymentSecret>,
	pub status: HTLCStatus,
	pub amt_msat: MillisatAmount,
}

pub type PaymentInfoStorage = Arc<Mutex<HashMap<PaymentHash, PaymentInfo>>>;

pub type ChainMonitor = chainmonitor::ChainMonitor<
	InMemorySigner,
	Arc<dyn Filter + Send + Sync>,
	Arc<BitcoindClient>,
	Arc<BitcoindClient>,
	Arc<FilesystemLogger>,
	Arc<FilesystemPersister>,
>;

pub(crate) type PeerManager = SimpleArcPeerManager<
	SocketDescriptor,
	ChainMonitor,
	BitcoindClient,
	BitcoindClient,
	dyn chain::Access + Send + Sync,
	FilesystemLogger,
>;

pub type ChannelManager =
	SimpleArcChannelManager<ChainMonitor, BitcoindClient, BitcoindClient, FilesystemLogger>;

pub type InvoicePayer<E> = payment::InvoicePayer<
	Arc<ChannelManager>,
	Router,
	Arc<Mutex<ProbabilisticScorer<Arc<NetworkGraph>>>>,
	Arc<FilesystemLogger>,
	E,
>;

pub type Router = DefaultRouter<Arc<NetworkGraph>, Arc<FilesystemLogger>>;

pub struct DataPersister {
	pub data_dir: String,
}

impl
	Persister<
		InMemorySigner,
		Arc<ChainMonitor>,
		Arc<BitcoindClient>,
		Arc<KeysManager>,
		Arc<BitcoindClient>,
		Arc<FilesystemLogger>,
	> for DataPersister
{
	fn persist_manager(&self, channel_manager: &ChannelManager) -> Result<(), std::io::Error> {
		FilesystemPersister::persist_manager(self.data_dir.clone(), channel_manager)
	}

	fn persist_graph(&self, network_graph: &NetworkGraph) -> Result<(), std::io::Error> {
		if FilesystemPersister::persist_network_graph(self.data_dir.clone(), network_graph).is_err()
		{
			// Persistence errors here are non-fatal as we can just fetch the routing graph
			// again later, but they may indicate a disk error which could be fatal elsewhere.
			eprintln!("Warning: Failed to persist network graph, check your disk and permissions");
		}

		Ok(())
	}
}
