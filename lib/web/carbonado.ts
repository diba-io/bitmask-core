// Methods meant to work with Carbonado storage defined within the web::carbonado module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export const store = async (
  nostrHexSk: string,
  data: Uint8Array,
  force: boolean,
  name?: string,
  meta?: Uint8Array
): Promise<void> => BMC.store(nostrHexSk, name || "", data, force, meta);

export const retrieve = (
  nostrHexSk: string,
  lookup: string
): Promise<Uint8Array> => BMC.retrieve(nostrHexSk, lookup);

export const retrieveMetadata = (
  nostrHexSk: string,
  lookup: string
): Promise<FileMetadata> => BMC.retrieve_metadata(nostrHexSk, lookup);

export const encodeHex = (bytes: Uint8Array): string => BMC.encode_hex(bytes);
export const encodeBase64 = (bytes: Uint8Array): string =>
  BMC.encode_base64(bytes);

export const decodeHex = (str: string): Uint8Array => BMC.decode_hex(str);
export const decodeBase64 = (str: string): Uint8Array => BMC.decode_base64(str);

export interface FileMetadata {
  filename: string;
  metadata: Uint8Array;
}
