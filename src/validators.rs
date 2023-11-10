use std::str::FromStr;

use bp::{Chain, Txid};
use miniscript_crate::Descriptor;
use rgbwallet::RgbInvoice;
use seals::txout::ExplicitSeal;
use wallet::hd::{DerivationAccount, DerivationSubpath, UnhardenedIndex};

use crate::structs::{IssueMediaRequest, SecretString};

/// Errors happening during checking of requests to RGB operations
#[derive(Clone, PartialEq, Eq, Debug, Display, Error, From)]
#[display(inner)]
pub enum RGBParamsError {
    /// wrong or unspecified seal closed method. Only TapRet (tapret1st)
    /// is supported
    #[display(doc_comments)]
    NoClosedMethod,

    /// the {0} need At least {1} media information to create RGB21 contracts
    #[display(doc_comments)]
    NoMediaType(String, u8),

    /// '{0}' is invalid terminal path (ex: /0/0)
    #[display(doc_comments)]
    WrongTerminal(String),

    /// '{0}' is invalid descriptor. {1}
    #[display(doc_comments)]
    WrongDescriptor(String, String),

    /// Rgb Invoice cannot be decoded. {0}
    WrongInvoice(String),
}

#[derive(Debug, Display)]
#[display(doc_comments)]
pub struct RGBContext {
    // Close Method supported
    closed_method: String,

    // Current Network
    current_network: String,

    // Minimum number of the media types (Only RGB21)
    min_media_types: u8,
}

impl Default for RGBContext {
    fn default() -> Self {
        Self {
            closed_method: "tapret1st".to_string(),
            current_network: String::new(),
            min_media_types: 1,
        }
    }
}

impl RGBContext {
    pub fn with(network: &str) -> Self {
        Self {
            current_network: network.to_string(),
            ..Default::default()
        }
    }
}

pub fn verify_tapret_seal(value: &str, context: &RGBContext) -> garde::Result {
    if !value.contains(&context.closed_method) {
        return Err(garde::Error::new(
            RGBParamsError::NoClosedMethod.to_string(),
        ));
    }
    ExplicitSeal::<Txid>::from_str(value).map_err(|op| garde::Error::new(op.to_string()))?;
    Ok(())
}

pub fn verify_terminal_path(value: &str, _context: &RGBContext) -> garde::Result {
    let resp = value
        .parse::<DerivationSubpath<UnhardenedIndex>>()
        .map_err(|op| RGBParamsError::WrongTerminal(op.to_string()));

    if resp.is_err() {
        return Err(garde::Error::new(resp.err().unwrap().to_string()));
    }

    Ok(())
}

pub fn verify_descriptor(value: &SecretString, _context: &RGBContext) -> garde::Result {
    let resp: Result<Descriptor<DerivationAccount>, _> = Descriptor::from_str(&value.to_string())
        .map_err(|op| RGBParamsError::WrongDescriptor(value.to_string(), op.to_string()));

    if resp.is_err() {
        return Err(garde::Error::new(resp.err().unwrap().to_string()));
    }

    Ok(())
}

pub fn verify_media_request(
    value: &Option<IssueMediaRequest>,
    context: &RGBContext,
) -> garde::Result {
    if let Some(request) = value {
        let mut media_type = 0;
        media_type += request.preview.is_some() as u8;
        media_type += request.media.is_some() as u8;
        media_type += request.attachments.len() as u8;

        if media_type < context.min_media_types {
            return Err(garde::Error::new(
                RGBParamsError::NoMediaType("UDA".to_string(), context.min_media_types).to_string(),
            ));
        };
    }
    Ok(())
}

pub fn verify_rgb_invoice(value: &str, context: &RGBContext) -> garde::Result {
    let rgb_invoice =
        RgbInvoice::from_str(value).map_err(|err| RGBParamsError::WrongInvoice(err.to_string()));

    if rgb_invoice.is_err() {
        return Err(garde::Error::new(rgb_invoice.err().unwrap().to_string()));
    }

    if let Some(chain) = rgb_invoice.unwrap().chain {
        let network = &context.current_network;
        let current_chain =
            Chain::from_str(network).map_err(|op| RGBParamsError::WrongInvoice(op.to_string()));

        if current_chain.is_err() {
            return Err(garde::Error::new(current_chain.err().unwrap().to_string()));
        }

        if current_chain.unwrap() != chain {
            return Err(garde::Error::new(
                RGBParamsError::WrongInvoice("Network mismatch".to_string()).to_string(),
            ));
        }
    }

    Ok(())
}
