// Default
pub const LIB_NAME_BITMASK: &str = "bitmask";
pub const RGB_CHANGE_INDEX: &str = "0";
pub const RGB_PSBT_TAPRET: &str = "TAPRET";
pub const RGB_DEFAULT_NAME: &str = "default";
pub const RGB_OLDEST_VERSION: [u8; 8] = [0; 8];
pub const RGB_STRICT_TYPE_VERSION: [u8; 8] = *b"rgbst161";

// General Errors
#[cfg(target_arch = "wasm32")]
pub const CARBONADO_UNAVALIABLE: &str = "carbonado filesystem";
#[cfg(not(target_arch = "wasm32"))]
pub const CARBONADO_UNAVALIABLE: &str = "carbonado server";
pub const STOCK_UNAVALIABLE: &str = "Unable to access Stock data";
pub const WALLET_UNAVALIABLE: &str = "Unable to access Wallet data";
