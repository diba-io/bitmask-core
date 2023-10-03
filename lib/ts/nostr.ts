// Methods meant to work with LNDHubX defined within the web::nostr module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export interface Response {
  status: string;
}

export const newNostrPubkey = async (
  pubkey: string,
  token: string
): Promise<Response> => JSON.parse(await BMC.new_nostr_pubkey(pubkey, token));

export const updateNostrPubkey = async (
  pubkey: string,
  token: string
): Promise<Response> =>
  JSON.parse(await BMC.update_nostr_pubkey(pubkey, token));
