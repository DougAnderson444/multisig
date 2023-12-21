use crate::{
    error::{AttributesError, SharesError},
    AttrId, AttrView, Builder, Error, Multisig, SigDataView, SigViews, ThresholdAttrView,
    ThresholdView,
};
use blsful::{vsss_rs::Share, Bls12381G1Impl, Bls12381G2Impl, Signature, SignatureShare};
use multicodec::Codec;
use multitrait::{EncodeInto, TryDecodeFrom};
use multiutil::{Varbytes, Varuint};
use std::{collections::BTreeMap, fmt};

#[repr(u8)]
#[derive(Clone, Copy, Default, Hash, Ord, PartialOrd, PartialEq, Eq)]
pub(crate) enum ShareTypeId {
    /// Basic
    #[default]
    Basic,
    /// Message Augmentation
    MessageAugmentation,
    /// ProofOfPossession
    ProofOfPossession,
}

impl ShareTypeId {
    /// Get the code for the attribute id
    pub fn code(&self) -> u8 {
        self.clone().into()
    }

    /// Convert the attribute id to &str
    pub fn as_str(&self) -> &str {
        match self {
            Self::Basic => "basic",
            Self::MessageAugmentation => "message-augmentation",
            Self::ProofOfPossession => "proof-of-possession",
        }
    }
}

impl Into<u8> for ShareTypeId {
    fn into(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for ShareTypeId {
    type Error = Error;

    fn try_from(c: u8) -> Result<Self, Self::Error> {
        match c {
            0 => Ok(Self::Basic),
            1 => Ok(Self::MessageAugmentation),
            2 => Ok(Self::ProofOfPossession),
            _ => Err(SharesError::InvalidShareTypeId(c).into()),
        }
    }
}

impl Into<Vec<u8>> for ShareTypeId {
    fn into(self) -> Vec<u8> {
        self.code().encode_into()
    }
}

impl<'a> TryFrom<&'a [u8]> for ShareTypeId {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let (id, _) = Self::try_decode_from(bytes)?;
        Ok(id)
    }
}

impl<'a> TryDecodeFrom<'a> for ShareTypeId {
    type Error = Error;

    fn try_decode_from(bytes: &'a [u8]) -> Result<(Self, &'a [u8]), Self::Error> {
        let (code, ptr) = u8::try_decode_from(bytes)?;
        Ok((Self::try_from(code)?, ptr))
    }
}

impl TryFrom<&str> for ShareTypeId {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_ascii_lowercase().as_str() {
            "basic" => Ok(Self::Basic),
            "message-augmentation" => Ok(Self::MessageAugmentation),
            "proof-of-possession" => Ok(Self::ProofOfPossession),
            _ => Err(SharesError::InvalidShareTypeName(s.to_string()).into()),
        }
    }
}

impl fmt::Display for ShareTypeId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone)]
pub(crate) struct SigShare(
    pub(crate) u8,          // identifier
    pub(crate) usize,       // threshold
    pub(crate) usize,       // limit
    pub(crate) ShareTypeId, // share type
    pub(crate) Vec<u8>,     // share bytes
);

impl Into<Vec<u8>> for SigShare {
    fn into(self) -> Vec<u8> {
        let mut v = Vec::default();
        // add in the share identifier
        v.append(&mut Varuint(self.0).into());
        // add in the share threshold
        v.append(&mut Varuint(self.1).into());
        // add in the share limit
        v.append(&mut Varuint(self.2).into());
        // add in the share type id
        v.append(&mut self.3.into());
        // add in the share data
        v.append(&mut Varbytes(self.4.clone()).into());
        v
    }
}

impl<'a> TryFrom<&'a [u8]> for SigShare {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let (share, _) = Self::try_decode_from(bytes)?;
        Ok(share)
    }
}

impl<'a> TryDecodeFrom<'a> for SigShare {
    type Error = Error;

    fn try_decode_from(bytes: &'a [u8]) -> Result<(Self, &'a [u8]), Self::Error> {
        // try to decode the identifier
        let (id, ptr) = Varuint::<u8>::try_decode_from(bytes)?;
        // try to decode the threshold
        let (threshold, ptr) = Varuint::<usize>::try_decode_from(ptr)?;
        // try to decode the limit
        let (limit, ptr) = Varuint::<usize>::try_decode_from(ptr)?;
        // try to decode the share type id
        let (share_type, ptr) = ShareTypeId::try_decode_from(ptr)?;
        // try to decode the share data
        let (share_data, ptr) = Varbytes::try_decode_from(ptr)?;
        Ok((
            Self(
                id.to_inner(),
                threshold.to_inner(),
                limit.to_inner(),
                share_type,
                share_data.to_inner(),
            ),
            ptr,
        ))
    }
}

#[derive(Clone, Default)]
pub(crate) struct ThresholdData(pub(crate) BTreeMap<u8, SigShare>);

impl Into<Vec<u8>> for ThresholdData {
    fn into(self) -> Vec<u8> {
        let mut v = Vec::default();
        // add in the number of sig shares
        v.append(&mut Varuint(self.0.len()).into());
        // add in the sig shares
        self.0.iter().for_each(|(_, share)| {
            v.append(&mut share.clone().into());
        });
        v
    }
}

impl<'a> TryFrom<&'a [u8]> for ThresholdData {
    type Error = Error;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        let (tdata, _) = Self::try_decode_from(bytes)?;
        Ok(tdata)
    }
}

impl<'a> TryDecodeFrom<'a> for ThresholdData {
    type Error = Error;

    fn try_decode_from(bytes: &'a [u8]) -> Result<(Self, &'a [u8]), Self::Error> {
        // try to decode the number of shares
        let (num_shares, ptr) = Varuint::<usize>::try_decode_from(bytes)?;
        // decode the signature-specific attributes
        let (shares, ptr) = match *num_shares {
            0 => (BTreeMap::default(), ptr),
            _ => {
                let mut shares = BTreeMap::new();
                let mut p = ptr;
                for _ in 0..*num_shares {
                    let (share, ptr) = SigShare::try_decode_from(p)?;
                    shares.insert(share.0, share);
                    p = ptr;
                }
                (shares, p)
            }
        };

        Ok((Self(shares), ptr))
    }
}

pub(crate) struct View<'a> {
    ms: &'a Multisig,
}

impl<'a> TryFrom<&'a Multisig> for View<'a> {
    type Error = Error;

    fn try_from(ms: &'a Multisig) -> Result<Self, Self::Error> {
        Ok(Self { ms })
    }
}

impl<'a> AttrView for View<'a> {
    /// for Bls Multisigs, the payload encoding is stored using the
    /// ShareTypeId::PayloadEncoding attribute id.
    fn payload_encoding(&self) -> Result<Codec, Error> {
        let v = self
            .ms
            .attributes
            .get(&AttrId::PayloadEncoding)
            .ok_or(AttributesError::MissingPayloadEncoding)?;
        let encoding = Codec::try_from(v.as_slice())?;
        Ok(encoding)
    }
}

impl<'a> SigDataView for View<'a> {
    /// For Bls Multisig values, the sig data is stored using the
    /// ShareTypeId::SigData attribute id.
    fn sig_bytes(&self) -> Result<Vec<u8>, Error> {
        let sig = self
            .ms
            .attributes
            .get(&AttrId::SigData)
            .ok_or(AttributesError::MissingSignature)?;
        Ok(sig.clone())
    }
}

impl<'a> ThresholdAttrView for View<'a> {
    /// get the threshold value for this multisig
    fn threshold(&self) -> Result<usize, Error> {
        let threshold = self
            .ms
            .attributes
            .get(&AttrId::Threshold)
            .ok_or(AttributesError::MissingThreshold)?;
        Ok(Varuint::<usize>::try_from(threshold.as_slice())?.to_inner())
    }
    /// get the limit value for this multisig
    fn limit(&self) -> Result<usize, Error> {
        let limit = self
            .ms
            .attributes
            .get(&AttrId::Limit)
            .ok_or(AttributesError::MissingLimit)?;
        Ok(Varuint::<usize>::try_from(limit.as_slice())?.to_inner())
    }
    /// get the share identifier
    fn identifier(&self) -> Result<u8, Error> {
        match self.ms.codec {
            Codec::Bls12381G1SigShare | Codec::Bls12381G2SigShare => {
                let identifier = self
                    .ms
                    .attributes
                    .get(&AttrId::ShareIdentifier)
                    .ok_or(AttributesError::MissingIdentifier)?;
                Ok(Varuint::<u8>::try_from(identifier.as_slice())?.to_inner())
            }
            _ => Err(SharesError::NotASignatureShare.into()),
        }
    }
}

/// trait for accumulating shares to rebuild a threshold signature
impl<'a> ThresholdView for View<'a> {
    /// get the signature shares
    fn shares(&self) -> Result<Vec<Multisig>, Error> {
        // get the codec for the new share multisigs
        let codec = match self.ms.codec {
            Codec::Bls12381G1Sig => Codec::Bls12381G1SigShare,
            Codec::Bls12381G2Sig => Codec::Bls12381G2SigShare,
            Codec::Bls12381G1SigShare | Codec::Bls12381G2SigShare => {
                return Err(SharesError::IsASignatureShare.into())
            }
            _ => return Err(Error::UnsupportedAlgorithm(self.ms.codec.to_string())),
        };

        // current Multisig threshold data
        let threshold_data = {
            let v = self.ms.attributes.get(&AttrId::ThresholdData);
            match v {
                Some(v) => ThresholdData::try_from(v.as_slice())?,
                None => ThresholdData::default(),
            }
        };

        // build the vec for the shares
        let mut shares = Vec::with_capacity(threshold_data.0.len());

        // build multisigs out of each share
        threshold_data
            .0
            .values()
            .try_for_each(|share| -> Result<(), Error> {
                let encoding = {
                    let av = self.ms.attr_view()?;
                    av.payload_encoding()?
                };
                let threshold_data: Vec<u8> = share.3.into();
                // build a multisig share out of the share, preserve the message
                // and the payload encoding value
                let share = Builder::new(codec)
                    .with_message_bytes(&self.ms.message.as_slice())
                    .with_identifier(share.0)
                    .with_threshold(share.1)
                    .with_limit(share.2)
                    .with_signature_bytes(&share.4)
                    .with_payload_encoding(encoding)
                    .with_threshold_data(&threshold_data)
                    .try_build()?;
                // add it to the list of shares
                shares.push(share);
                Ok(())
            })?;

        Ok(shares)
    }
    /// add a new share and return the Multisig with the share added
    fn add_share(&self, share: &Multisig) -> Result<Multisig, Error> {
        // check the codec is correct for this function
        match self.ms.codec {
            Codec::Bls12381G1Sig | Codec::Bls12381G2Sig => {}
            Codec::Bls12381G1SigShare | Codec::Bls12381G2SigShare => {
                return Err(SharesError::IsASignatureShare.into())
            }
            _ => return Err(Error::UnsupportedAlgorithm(self.ms.codec.to_string())),
        };

        let (sdata, identifier, threshold, limit, encoding) = {
            // get the share's share type out of the threshold data
            let v = share
                .attributes
                .get(&AttrId::ThresholdData)
                .ok_or(AttributesError::MissingThresholdData)?;
            let share_type = ShareTypeId::try_from(v.as_slice())?;

            // get the share's attributes
            let av = share.threshold_attr_view()?;
            let threshold = av.threshold()?;
            let limit = av.limit()?;
            let identifier = av.identifier()?;

            // get the share's signature data
            let dv = share.sig_data_view()?;
            let sig_bytes = dv.sig_bytes()?;

            let encoding = {
                let av = self.ms.attr_view()?;
                av.payload_encoding().ok()
            };

            // create the sig share tuple
            (
                SigShare(identifier, threshold, limit, share_type, sig_bytes),
                identifier,
                threshold,
                limit,
                encoding,
            )
        };

        // update the threshold data
        let threshold_data: Vec<u8> = {
            let v = self.ms.attributes.get(&AttrId::ThresholdData);
            let mut tdata = match v {
                Some(v) => ThresholdData::try_from(v.as_slice())?,
                None => ThresholdData::default(),
            };
            // insert the share data into the list of shares
            tdata.0.insert(identifier, sdata);
            tdata.into()
        };

        // get the payload encoding
        let encoding = {
            let av = self.ms.attr_view()?;
            // if this multisig doesn't have payload encoding set, set it to
            // the value from the first share added
            match av.payload_encoding() {
                Ok(encoding) => Some(encoding),
                Err(_) => {
                    if let Some(encoding) = encoding {
                        Some(encoding)
                    } else {
                        None
                    }
                }
            }
        };

        // if this multisig doesn't already have the threshold/limit set then
        // set it to match the values from the first share added
        let av = share.threshold_attr_view()?;
        let threshold = av.threshold().unwrap_or(threshold);
        let limit = av.limit().unwrap_or(limit);

        let builder = Builder::new(self.ms.codec)
            .with_message_bytes(&self.ms.message.as_slice())
            .with_threshold(threshold)
            .with_limit(limit)
            .with_threshold_data(&threshold_data);

        if let Some(encoding) = encoding {
            builder.with_payload_encoding(encoding).try_build()
        } else {
            builder.try_build()
        }
    }
    /// reconstruct the signature from the shares
    fn combine(&self) -> Result<Multisig, Error> {
        // current Multisig threshold data
        let threshold_data = {
            let v = self.ms.attributes.get(&AttrId::ThresholdData);
            match v {
                Some(v) => ThresholdData::try_from(v.as_slice())?,
                None => ThresholdData::default(),
            }
        };

        // check that we have enough shares to combine
        let num_shares = threshold_data.0.len();
        let av = self.ms.threshold_attr_view()?;
        if num_shares < av.threshold()? {
            return Err(SharesError::NotEnoughShares.into());
        }

        match self.ms.codec {
            Codec::Bls12381G1Sig => {
                let mut share_type_id: Option<ShareTypeId> = None;
                let mut shares = Vec::default();
                threshold_data
                    .0
                    .iter()
                    .try_for_each(|(id, share)| -> Result<(), Error> {
                        let vsss = Share::with_identifier_and_value(*id, share.4.as_slice());
                        // check to make sure all of the shares are of the same type
                        if let Some(sti) = share_type_id {
                            if sti != share.3 {
                                return Err(SharesError::ShareTypeMismatch.into());
                            }
                        } else {
                            share_type_id = Some(share.3);
                        }
                        let s = match share.3 {
                            ShareTypeId::Basic => SignatureShare::<Bls12381G1Impl>::Basic(vsss),
                            ShareTypeId::MessageAugmentation => {
                                SignatureShare::<Bls12381G1Impl>::MessageAugmentation(vsss)
                            }
                            ShareTypeId::ProofOfPossession => {
                                SignatureShare::<Bls12381G1Impl>::ProofOfPossession(vsss)
                            }
                        };
                        shares.push(s);
                        Ok(())
                    })?;

                let sig = Signature::from_shares(shares.as_slice())
                    .map_err(|e| SharesError::ShareCombineFailed(e.to_string()))?;
                let encoding = {
                    let av = self.ms.attr_view()?;
                    av.payload_encoding()?
                };
                Builder::new_from_bls_signature(&sig)?
                    .with_message_bytes(&self.ms.message.as_slice())
                    .with_payload_encoding(encoding)
                    .try_build()
            }
            Codec::Bls12381G2Sig => {
                let mut share_type_id: Option<ShareTypeId> = None;
                let mut shares = Vec::default();
                threshold_data
                    .0
                    .iter()
                    .try_for_each(|(id, share)| -> Result<(), Error> {
                        let vsss = Share::with_identifier_and_value(*id, share.4.as_slice());
                        // check to make sure all of the shares are of the same type
                        if let Some(sti) = share_type_id {
                            if sti != share.3 {
                                return Err(SharesError::ShareTypeMismatch.into());
                            }
                        } else {
                            share_type_id = Some(share.3);
                        }
                        let s = match share.3 {
                            ShareTypeId::Basic => SignatureShare::<Bls12381G2Impl>::Basic(vsss),
                            ShareTypeId::MessageAugmentation => {
                                SignatureShare::<Bls12381G2Impl>::MessageAugmentation(vsss)
                            }
                            ShareTypeId::ProofOfPossession => {
                                SignatureShare::<Bls12381G2Impl>::ProofOfPossession(vsss)
                            }
                        };
                        shares.push(s);
                        Ok(())
                    })?;

                let sig = Signature::from_shares(shares.as_slice())
                    .map_err(|e| SharesError::ShareCombineFailed(e.to_string()))?;
                let encoding = {
                    let av = self.ms.attr_view()?;
                    av.payload_encoding().ok()
                };
                let builder = Builder::new_from_bls_signature(&sig)?
                    .with_message_bytes(&self.ms.message.as_slice());

                if let Some(encoding) = encoding {
                    builder.with_payload_encoding(encoding).try_build()
                } else {
                    builder.try_build()
                }
            }
            _ => return Err(Error::UnsupportedAlgorithm(self.ms.codec.to_string())),
        }
    }
}
