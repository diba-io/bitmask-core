// Methods meant to work with bitmask-core constants defined within the web::constants module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import initBMC, * as BMC from "./bitmask_core";

export const getNetwork = async (): Promise<string> =>
  JSON.parse(await BMC.get_network());

export const switchNetwork = async (network: Network): Promise<void> =>
  BMC.switch_network(network.toString());

export const getEnv = async (key: string): Promise<string> =>
  JSON.parse(await BMC.get_env(key));

export const setEnv = async (key: string, value: string): Promise<void> =>
  BMC.set_env(key, value);

export enum Network {
  bitcoin = "bitcoin",
  testnet = "testnet",
  signet = "signet",
  regtest = "regtest",
}
type NetworkType = typeof Network;
type NetworkKeyType = keyof NetworkType;

export const DISABLE_LN =
  process.env?.DISABLE_LN === "true" ? true : false || "";
export let LNDHUBX = false;
export let CARBONADO = false;
export let BITMASK = false;

export const init = async (networkOverride?: string) => {
  try {
    await initBMC();

    if (networkOverride)
      window.localStorage.setItem("network", networkOverride);
    const storedNetwork =
      networkOverride || window.localStorage.getItem("network");
    if (storedNetwork) {
      await switchNetwork(Network[storedNetwork as NetworkKeyType]);
    } else {
      window.localStorage.setItem("network", Network.bitcoin);
      await switchNetwork(Network.bitcoin);
    }

    const network = await getNetwork();
    if (network === "bitcoin" && process.env.PROD_LNDHUB_ENDPOINT) {
      await setEnv("LNDHUB_ENDPOINT", process.env.PROD_LNDHUB_ENDPOINT);
    } else if (process.env.TEST_LNDHUB_ENDPOINT) {
      await setEnv("LNDHUB_ENDPOINT", process.env.TEST_LNDHUB_ENDPOINT);
    }
    if (process.env.CARBONADO_ENDPOINT) {
      await setEnv("CARBONADO_ENDPOINT", process.env.CARBONADO_ENDPOINT);
    }
    if (process.env.BITMASK_ENDPOINT) {
      await setEnv("BITMASK_ENDPOINT", process.env.BITMASK_ENDPOINT);
    }
    if (process.env.BITCOIN_EXPLORER_API_MAINNET) {
      await setEnv(
        "BITCOIN_EXPLORER_API_MAINNET",
        process.env.BITCOIN_EXPLORER_API_MAINNET
      );
    }
  } catch (err) {
    console.error("Error in setEnv", err);
  }

  const lndhubx = await getEnv("LNDHUB_ENDPOINT");
  const carbonado = await getEnv("CARBONADO_ENDPOINT");
  const bitmask = await getEnv("BITMASK_ENDPOINT");

  try {
    await fetch(`${lndhubx}/nodeinfo`);
    LNDHUBX = true;
    console.debug(`${lndhubx}/nodeinfo successfully reached`);
  } catch (e) {
    LNDHUBX = false;
    console.warn("Could not reach lndhubx", lndhubx, e);
  }
  try {
    await fetch(`${carbonado}/status`);
    CARBONADO = true;
    console.debug(`${carbonado}/status successfully reached`);
  } catch (e) {
    CARBONADO = false;
    console.warn("Could not reach carbonado", carbonado, e);
  }
  try {
    await fetch(`${bitmask}/carbonado/status`);
    BITMASK = true;
    console.debug(`${bitmask}/status successfully reached`);
  } catch (e) {
    BITMASK = false;
    console.warn("Could not reach bitmask", bitmask, e);
  }

  console.debug("Using LNDHubX endpoint:", lndhubx);
  console.debug("Using Carbonado endpoint:", carbonado);
  console.debug("Using bitmaskd endpoint:", bitmask);
};

init();
