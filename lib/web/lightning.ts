// Methods meant to work with LNDHubX defined within the web::lightning module from bitmask-core:
// https://github.com/diba-io/bitmask-core/blob/development/src/web.rs

import * as BMC from "./bitmask_core";

export const createWallet = async (
  username: string,
  password: string
): Promise<CreateWalletResponse> =>
  JSON.parse(await BMC.create_wallet(username, password));

export const auth = async (
  username: string,
  password: string
): Promise<AuthResponse> => JSON.parse(await BMC.auth(username, password));

export const createInvoice = async (
  description: string,
  amount: number,
  token: string
): Promise<AddInvoiceResponse> =>
  JSON.parse(await BMC.ln_create_invoice(description, amount, token));

export const getBalance = async (token: string): Promise<Account> =>
  JSON.parse(await BMC.get_balance(token));

export const getTxs = async (token: string): Promise<LnTransaction[]> =>
  JSON.parse(await BMC.get_txs(token));

export const payInvoice = async (
  paymentRequest: string,
  token: string
): Promise<PayInvoiceResponse> =>
  JSON.parse(await BMC.pay_invoice(paymentRequest, token));

export const checkPayment = async (
  paymentHash: string
): Promise<CheckPaymentResponse> =>
  JSON.parse(await BMC.check_payment(paymentHash));

export const swapBtcLn = async (token: string): Promise<SwapBtcLnResponse> =>
  JSON.parse(await BMC.swap_btc_ln(token));

export const swapLnBtc = async (
  address: string,
  amount: bigint,
  token: string
): Promise<SwapLnBtcResponse> =>
  JSON.parse(await BMC.swap_ln_btc(address, amount, token));

// Core type interfaces based on structs defined within the bitmask-core Rust crate:
// https://github.com/diba-io/bitmask-core/blob/development/src/structs.rs

export interface LnCredentials {
  login: string;
  password: string;
  refreshToken: string;
  accessToken: string;
}

// Lndhubx Create wallet endpoint response
export interface CreateWalletResponse {
  username?: string;
  error?: string;
}

// lndhubx Auth response
export type AuthResponse = ErrorResponse | AuthResponseOk;

export interface AuthResponseOk {
  refresh: string;
  token: string;
}

export interface ErrorResponse {
  error: string;
}

// User Account
export interface Account {
  account_id: string;
  balance: string;
  currency: string;
}

// Amount and currency
export interface Money {
  value: string;
  currency: string;
}

// Lndhubx Add invoice endpoint response
export interface AddInvoiceResponse {
  req_id: string;
  uid: number;
  payment_request: string;
  meta: string;
  metadata: string;
  amount: Money;
  rate: string;
  currency: string;
  target_account_currency: string;
  account_id: string;
  error: string;
  fees: string;
}

// Lndhubx lightning transaction
export interface LnTransaction {
  txid: string;
  fee_txid: string;
  outbound_txid: string;
  inbound_txid: string;
  created_at: bigint;
  date: number;
  outbound_amount: string;
  inbound_amount: string;
  outbound_account_id: string;
  inbound_account_id: string;
  outbound_uid: number;
  inbound_uid: number;
  outbound_currency: string;
  inbound_currency: string;
  exchange_rate: string;
  tx_type: string;
  fees: string;
  reference: string;
}

export interface LnWalletData {
  balance: Account;
  transactions: LnTransaction[];
}

// Lndhubx Pay invoice response
export interface PayInvoiceResponse {
  payment_hash: string;
  uid: number;
  success: boolean;
  currency: string;
  payment_request: string;
  amount: Money;
  fees: Money;
  error: string;
  payment_preimage: string;
  destination: string;
  description: string;
}

// Lndhubx Check payment response
export interface CheckPaymentResponse {
  paid: boolean;
}

export interface SwapBtcLnResponse {
  address: string;
  commitment: string;
  signature: string;
  secret_access_key: string;
}

export interface SwapLnBtcResponse {
  bolt11_invoice: string;
  fee_sats: number;
}
