use std::{
    path::PathBuf,
    str::FromStr,
    sync::mpsc::{self, SyncSender},
};

use anyhow::Result;
use tokio::task::spawn_blocking;

use crate::{data::constants::BITCOIN_ELECTRUM_API, debug, error, info, trace};

pub async fn rgb_init() -> SyncSender<i32> {
    let (tx, rx) = mpsc::sync_channel::<i32>(1);

    let stored = spawn_blocking(|| {
        stored().expect("start stored");
    });
    let stormd = spawn_blocking(|| {
        stormd().expect("start stormd");
    });
    let rgbd = spawn_blocking(|| {
        rgbd().expect("start stormd");
    });

    // Await oneshot to abort threads

    // tokio::spawn(async move {
    //     match rx.recv().await {
    spawn_blocking(move || {
        match rx.recv() {
            Ok(shutdown) => {
                info!("Aborting RGB daemon threads");
                stored.abort();
                stormd.abort();
                rgbd.abort();
                info!("All threads aborted. Exiting...");
                std::process::exit(shutdown);
            }
            Err(e) => {
                error!(format!("Error in rgb_init abort: {e}"));
                panic!("Error in rgb_init abort");
            }
        };
    });

    tx
}

use internet2::addr::ServiceAddr;

pub fn inproc(name: &str) -> ServiceAddr {
    ServiceAddr::Inproc(name.to_owned())
}

fn stored() -> Result<()> {
    use stored::{service, Config};
    info!("stored: storage microservice");

    let mut config = Config {
        rpc_endpoint: inproc("stored"),
        data_dir: PathBuf::from_str("/tmp/.storm_node")?,
        databases: Default::default(),
        verbose: 3,
    };
    trace!(format!("Daemon configuration: {:?}", config));
    config.process();
    trace!(format!("Processed configuration: {:?}", config));
    debug!(format!("CTL RPC socket {}", config.rpc_endpoint));

    debug!("Starting runtime ...");
    service::run(config).expect("running stored runtime");

    Ok(())
}

fn stormd() -> Result<()> {
    use storm_node::{stormd, Config};
    info!("stored: storage microservice");

    let config: Config<stormd::Config> = Config {
        data_dir: PathBuf::from_str("/tmp/.storm_node")?,
        msg_endpoint: inproc("storm_msg"),
        ctl_endpoint: inproc("stormd_ctl"),
        rpc_endpoint: inproc("stormd"),
        ext_endpoint: inproc("stormext"),
        store_endpoint: inproc("stored"),
        chat_endpoint: inproc("chatd"),
        ext: stormd::Config {
            run_chat: false,
            run_downpour: false,
            threaded: true,
        },
    };
    trace!(format!("Daemon configuration: {:?}", config));
    debug!(format!("MSG socket {}", config.msg_endpoint));
    debug!(format!("CTL socket {}", config.ctl_endpoint));
    debug!(format!("RPC socket {}", config.rpc_endpoint));
    debug!(format!("STORM socket {}", config.ext_endpoint));
    debug!(format!("STORE socket {}", config.store_endpoint));
    debug!(format!("CHAT socket {}", config.chat_endpoint));

    debug!("Starting runtime ...");
    stormd::run(config).expect("running stromd runtime");

    Ok(())
}

fn rgbd() -> Result<()> {
    use lnpbp::chain::Chain;
    use rgb_node::{rgbd, Config};
    info!("rgbd: RGB stash microservice");

    let config = Config {
        rpc_endpoint: inproc("rgbd"),
        ctl_endpoint: inproc("rgbd_ctl"),
        storm_endpoint: inproc("stormd"),
        store_endpoint: inproc("stored"),
        data_dir: PathBuf::from_str("/tmp/.rgb_node")?,
        electrum_url: BITCOIN_ELECTRUM_API.read().unwrap().to_owned(),
        chain: Chain::Testnet3, // TODO: Determine at runtime
        threaded: true,
    };
    trace!(format!("Daemon configuration: {:?}", config));
    debug!(format!("CTL socket {}", config.ctl_endpoint));
    debug!(format!("RPC socket {}", config.rpc_endpoint));
    debug!(format!("STORE socket {}", config.store_endpoint));
    debug!(format!("STORM socket {}", config.storm_endpoint));

    debug!("Starting runtime ...");
    rgbd::run(config).expect("running rgbd runtime");

    Ok(())
}
