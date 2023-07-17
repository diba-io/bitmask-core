use std::str::FromStr;

use bp::Txid;
use miniscript_crate::Descriptor;
use seals::txout::ExplicitSeal;
use wallet::hd::{DerivationAccount, DerivationSubpath, UnhardenedIndex};

use crate::structs::{IssueMetaRequest, IssueMetadata, SecretString};

/// Errors happening during checking of requests to RGB operations
#[derive(Clone, PartialEq, Eq, Debug, Display, Error, From)]
#[display(inner)]
pub enum RGBParamsError {
    /// wrong or unspecified seal closed method. Only TapRet (tapret1st)
    /// is
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
}

#[derive(Debug, Display)]
#[display(doc_comments)]
pub struct RGBContext {
    // Close Method supported
    closed_method: String,

    // Minimum number of the media types (Only RGB21)
    min_media_types: u8,
}

impl Default for RGBContext {
    fn default() -> Self {
        Self {
            closed_method: "tapret1st".to_string(),
            min_media_types: 1,
        }
    }
}

pub fn is_tapret_seal(value: &str, context: &RGBContext) -> garde::Result {
    if !value.contains(&context.closed_method) {
        return Err(garde::Error::new(
            RGBParamsError::NoClosedMethod.to_string(),
        ));
    }
    ExplicitSeal::<Txid>::from_str(value).map_err(|op| garde::Error::new(op.to_string()))?;
    Ok(())
}

pub fn is_terminal_path(value: &str, _context: &RGBContext) -> garde::Result {
    let resp = value
        .parse::<DerivationSubpath<UnhardenedIndex>>()
        .map_err(|op| RGBParamsError::WrongTerminal(op.to_string()));

    if resp.is_err() {
        return Err(garde::Error::new(resp.err().unwrap().to_string()));
    }

    Ok(())
}

pub fn is_descriptor(value: &SecretString, _context: &RGBContext) -> garde::Result {
    let resp: Result<Descriptor<DerivationAccount>, _> = Descriptor::from_str(&value.to_string())
        .map_err(|op| RGBParamsError::WrongDescriptor(value.to_string(), op.to_string()));

    if resp.is_err() {
        return Err(garde::Error::new(resp.err().unwrap().to_string()));
    }

    Ok(())
}

pub fn has_media_types(value: &Option<IssueMetaRequest>, context: &RGBContext) -> garde::Result {
    if let Some(metadata) = value {
        match &metadata.0 {
            IssueMetadata::UDA(media_type) => {
                if media_type.len() < context.min_media_types.into() {
                    return Err(garde::Error::new(
                        RGBParamsError::NoMediaType("UDA".to_string(), context.min_media_types)
                            .to_string(),
                    ));
                };
            }
            IssueMetadata::Collectible(items) => {
                for (i, item) in items.iter().enumerate() {
                    if item.media.len() < context.min_media_types.into() {
                        return Err(garde::Error::new(
                            RGBParamsError::NoMediaType(
                                format!("collectible item #{}", i),
                                context.min_media_types,
                            )
                            .to_string(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
