// Copyright 2015-2023 Benjamin Fry <benjaminfry@me.com>
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! public key record data for signing zone records

use alloc::{borrow::ToOwned, sync::Arc, vec::Vec};
use core::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    dnssec::{
        Algorithm, DigestType, PublicKey, PublicKeyBuf, Verifier,
        crypto::{Digest, decode_public_key},
    },
    error::{ProtoError, ProtoErrorKind, ProtoResult},
    rr::{Name, RecordData, RecordDataDecodable, RecordType, record_data::RData},
    serialize::binary::{
        BinDecodable, BinDecoder, BinEncodable, BinEncoder, NameEncoding, Restrict, RestrictedMath,
    },
};

use super::DNSSECRData;

/// [RFC 4034](https://tools.ietf.org/html/rfc4034#section-2), DNSSEC Resource Records, March 2005
///
/// ```text
/// 2.  The DNSKEY Resource Record
///
///    DNSSEC uses public key cryptography to sign and authenticate DNS
///    resource record sets (RRsets).  The public keys are stored in DNSKEY
///    resource records and are used in the DNSSEC authentication process
///    described in [RFC4035]: A zone signs its authoritative RRsets by
///    using a private key and stores the corresponding public key in a
///    DNSKEY RR.  A resolver can then use the public key to validate
///    signatures covering the RRsets in the zone, and thus to authenticate
///    them.
///
///    The DNSKEY RR is not intended as a record for storing arbitrary
///    public keys and MUST NOT be used to store certificates or public keys
///    that do not directly relate to the DNS infrastructure.
///
///    The Type value for the DNSKEY RR type is 48.
///
///    The DNSKEY RR is class independent.
///
///    The DNSKEY RR has no special TTL requirements.
///
/// 2.1.  DNSKEY RDATA Wire Format
///
///    The RDATA for a DNSKEY RR consists of a 2 octet Flags Field, a 1
///    octet Protocol Field, a 1 octet Algorithm Field, and the Public Key
///    Field.
///
///                         1 1 1 1 1 1 1 1 1 1 2 2 2 2 2 2 2 2 2 2 3 3
///     0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
///    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///    |              Flags            |    Protocol   |   Algorithm   |
///    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///    /                                                               /
///    /                            Public Key                         /
///    /                                                               /
///    +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
///
/// 2.1.5.  Notes on DNSKEY RDATA Design
///
///    Although the Protocol Field always has value 3, it is retained for
///    backward compatibility with early versions of the KEY record.
///
/// ```
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct DNSKEY {
    pub flags: u16,
    pub public_key: PublicKeyBuf,
}

impl DNSKEY {
    /// Create a [`DNSKEY`] record representing a `public_key`.
    ///
    /// # Arguments
    ///
    /// * `algorithm` - algorithm of the DNSKEY
    ///
    /// # Return
    ///
    /// the DNSKEY record data
    pub fn from_key(public_key: &dyn PublicKey) -> Self {
        Self::new(
            true,
            true,
            false,
            PublicKeyBuf::new(public_key.public_bytes().to_owned(), public_key.algorithm()),
        )
    }

    /// Construct a new DNSKey RData
    ///
    /// # Arguments
    ///
    /// * `zone_key` - this key is used to sign Zone resource records
    /// * `secure_entry_point` - this key is used to sign DNSKeys that sign the Zone records
    /// * `revoke` - this key has been revoked
    /// * `public_key` - the public key
    ///
    /// # Return
    ///
    /// A new DNSKEY RData for use in a Resource Record
    pub fn new(
        zone_key: bool,
        secure_entry_point: bool,
        revoke: bool,
        public_key: PublicKeyBuf,
    ) -> Self {
        let mut flags: u16 = 0;
        if zone_key {
            flags |= 0b0000_0001_0000_0000;
        }
        if secure_entry_point {
            flags |= 0b0000_0000_0000_0001;
        }
        if revoke {
            flags |= 0b0000_0000_1000_0000;
        }
        Self::with_flags(flags, public_key)
    }

    /// Construct a new DNSKEY RData
    ///
    /// # Arguments
    ///
    /// * `flags` - flags associated with this key
    /// * `public_key` - the public key
    ///
    /// # Return
    ///
    /// A new DNSKEY RData for use in a Resource Record
    pub fn with_flags(flags: u16, public_key: PublicKeyBuf) -> Self {
        Self { flags, public_key }
    }

    /// [RFC 4034, DNSSEC Resource Records, March 2005](https://tools.ietf.org/html/rfc4034#section-2.1.1)
    ///
    /// ```text
    /// 2.1.1.  The Flags Field
    ///
    ///    Bit 7 of the Flags field is the Zone Key flag.  If bit 7 has value 1,
    ///    then the DNSKEY record holds a DNS zone key, and the DNSKEY RR's
    ///    owner name MUST be the name of a zone.  If bit 7 has value 0, then
    ///    the DNSKEY record holds some other type of DNS public key and MUST
    ///    NOT be used to verify RRSIGs that cover RRsets.
    ///
    ///
    ///    Bits 0-6 and 8-14 are reserved: these bits MUST have value 0 upon
    ///    creation of the DNSKEY RR and MUST be ignored upon receipt.
    /// ```
    pub fn zone_key(&self) -> bool {
        self.flags & 0b0000_0001_0000_0000 != 0
    }

    /// [RFC 4034, DNSSEC Resource Records, March 2005](https://tools.ietf.org/html/rfc4034#section-2.1.1)
    ///
    /// ```text
    /// 2.1.1.  The Flags Field
    ///
    ///    Bit 15 of the Flags field is the Secure Entry Point flag, described
    ///    in [RFC3757].  If bit 15 has value 1, then the DNSKEY record holds a
    ///    key intended for use as a secure entry point.  This flag is only
    ///    intended to be a hint to zone signing or debugging software as to the
    ///    intended use of this DNSKEY record; validators MUST NOT alter their
    ///    behavior during the signature validation process in any way based on
    ///    the setting of this bit.  This also means that a DNSKEY RR with the
    ///    SEP bit set would also need the Zone Key flag set in order to be able
    ///    to generate signatures legally.  A DNSKEY RR with the SEP set and the
    ///    Zone Key flag not set MUST NOT be used to verify RRSIGs that cover
    ///    RRsets.
    /// ```
    pub fn secure_entry_point(&self) -> bool {
        self.flags & 0b0000_0000_0000_0001 != 0
    }

    /// A KSK has a `flags` value of `257`
    pub fn is_key_signing_key(&self) -> bool {
        // a flags value of 257
        self.secure_entry_point() && self.zone_key() && !self.revoke()
    }

    /// [RFC 5011, Trust Anchor Update, September 2007](https://tools.ietf.org/html/rfc5011#section-3)
    ///
    /// ```text
    /// RFC 5011                  Trust Anchor Update             September 2007
    ///
    /// 7.  IANA Considerations
    ///
    ///   The IANA has assigned a bit in the DNSKEY flags field (see Section 7
    ///   of [RFC4034]) for the REVOKE bit (8).
    /// ```
    pub fn revoke(&self) -> bool {
        self.flags & 0b0000_0000_1000_0000 != 0
    }

    /// The [`PublicKeyBuf`] type combines the algorithm and the public key material.
    ///
    /// [RFC 4034, DNSSEC Resource Records, March 2005](https://tools.ietf.org/html/rfc4034#section-2.1.4)
    ///
    /// ```text
    /// 2.1.3.  The Algorithm Field
    ///
    ///    The Algorithm field identifies the public key's cryptographic
    ///    algorithm and determines the format of the Public Key field.  A list
    ///    of DNSSEC algorithm types can be found in Appendix A.1
    ///
    /// 2.1.4.  The Public Key Field
    ///
    ///    The Public Key Field holds the public key material.  The format
    ///    depends on the algorithm of the key being stored and is described in
    ///    separate documents.
    /// ```
    pub fn public_key(&self) -> &PublicKeyBuf {
        &self.public_key
    }

    /// Output the encoded form of the flags
    pub fn flags(&self) -> u16 {
        self.flags
    }

    /// Creates a message digest for this DNSKEY record.
    ///
    /// ```text
    /// 5.1.4.  The Digest Field
    ///
    ///    The DS record refers to a DNSKEY RR by including a digest of that
    ///    DNSKEY RR.
    ///
    ///    The digest is calculated by concatenating the canonical form of the
    ///    fully qualified owner name of the DNSKEY RR with the DNSKEY RDATA,
    ///    and then applying the digest algorithm.
    ///
    ///      digest = digest_algorithm( DNSKEY owner name | DNSKEY RDATA);
    ///
    ///       "|" denotes concatenation
    ///
    ///      DNSKEY RDATA = Flags | Protocol | Algorithm | Public Key.
    ///
    ///    The size of the digest may vary depending on the digest algorithm and
    ///    DNSKEY RR size.  As of the time of this writing, the only defined
    ///    digest algorithm is SHA-1, which produces a 20 octet digest.
    /// ```
    ///
    /// # Arguments
    ///
    /// * `name` - the label of of the DNSKEY record.
    /// * `digest_type` - the `DigestType` with which to create the message digest.
    pub fn to_digest(&self, name: &Name, digest_type: DigestType) -> ProtoResult<Digest> {
        let mut buf: Vec<u8> = Vec::new();
        {
            let mut encoder: BinEncoder<'_> = BinEncoder::new(&mut buf);
            encoder.set_name_encoding(NameEncoding::UncompressedLowercase);
            if let Err(e) = name
                .to_lowercase()
                .emit(&mut encoder)
                .and_then(|_| self.emit(&mut encoder))
            {
                tracing::warn!("error serializing dnskey: {e}");
                return Err(format!("error serializing dnskey: {e}").into());
            }
        }

        Digest::new(&buf, digest_type)
    }

    /// The key tag is calculated as a hash to more quickly lookup a DNSKEY.
    ///
    /// [RFC 2535](https://tools.ietf.org/html/rfc2535), Domain Name System Security Extensions, March 1999
    ///
    /// ```text
    /// RFC 2535                DNS Security Extensions               March 1999
    ///
    /// 4.1.6 Key Tag Field
    ///
    ///  The "key Tag" is a two octet quantity that is used to efficiently
    ///  select between multiple keys which may be applicable and thus check
    ///  that a public key about to be used for the computationally expensive
    ///  effort to check the signature is possibly valid.  For algorithm 1
    ///  (MD5/RSA) as defined in [RFC 2537], it is the next to the bottom two
    ///  octets of the public key modulus needed to decode the signature
    ///  field.  That is to say, the most significant 16 of the least
    ///  significant 24 bits of the modulus in network (big endian) order. For
    ///  all other algorithms, including private algorithms, it is calculated
    ///  as a simple checksum of the KEY RR as described in Appendix C.
    ///
    /// Appendix C: Key Tag Calculation
    ///
    ///  The key tag field in the SIG RR is just a means of more efficiently
    ///  selecting the correct KEY RR to use when there is more than one KEY
    ///  RR candidate available, for example, in verifying a signature.  It is
    ///  possible for more than one candidate key to have the same tag, in
    ///  which case each must be tried until one works or all fail.  The
    ///  following reference implementation of how to calculate the Key Tag,
    ///  for all algorithms other than algorithm 1, is in ANSI C.  It is coded
    ///  for clarity, not efficiency.  (See section 4.1.6 for how to determine
    ///  the Key Tag of an algorithm 1 key.)
    ///
    ///  /* assumes int is at least 16 bits
    ///     first byte of the key tag is the most significant byte of return
    ///     value
    ///     second byte of the key tag is the least significant byte of
    ///     return value
    ///     */
    ///
    ///  int keytag (
    ///
    ///          unsigned char key[],  /* the RDATA part of the KEY RR */
    ///          unsigned int keysize, /* the RDLENGTH */
    ///          )
    ///  {
    ///  long int    ac;    /* assumed to be 32 bits or larger */
    ///
    ///  for ( ac = 0, i = 0; i < keysize; ++i )
    ///      ac += (i&1) ? key[i] : key[i]<<8;
    ///  ac += (ac>>16) & 0xFFFF;
    ///  return ac & 0xFFFF;
    ///  }
    /// ```
    pub fn calculate_key_tag(&self) -> ProtoResult<u16> {
        // TODO:
        let mut bytes: Vec<u8> = Vec::with_capacity(512);
        {
            let mut e = BinEncoder::new(&mut bytes);
            self.emit(&mut e)?;
        }
        Ok(Self::calculate_key_tag_internal(&bytes))
    }

    /// Internal checksum function (used for non-RSAMD5 hashes only,
    /// however, RSAMD5 is considered deprecated and not implemented in
    /// hickory-dns, anyways).
    pub fn calculate_key_tag_internal(bytes: &[u8]) -> u16 {
        let mut ac: u32 = 0;
        for (i, k) in bytes.iter().enumerate() {
            ac += u32::from(*k) << if i & 0x01 != 0 { 0 } else { 8 };
        }
        ac += ac >> 16;
        (ac & 0xFFFF) as u16
    }
}

impl From<DNSKEY> for RData {
    fn from(key: DNSKEY) -> Self {
        Self::DNSSEC(super::DNSSECRData::DNSKEY(key))
    }
}

impl BinEncodable for DNSKEY {
    fn emit(&self, encoder: &mut BinEncoder<'_>) -> ProtoResult<()> {
        encoder.emit_u16(self.flags())?;
        encoder.emit(3)?; // always 3 for now
        self.public_key.algorithm().emit(encoder)?;
        encoder.emit_vec(self.public_key.public_bytes())?;

        Ok(())
    }
}

impl<'r> RecordDataDecodable<'r> for DNSKEY {
    fn read_data(decoder: &mut BinDecoder<'r>, length: Restrict<u16>) -> ProtoResult<Self> {
        let flags: u16 = decoder.read_u16()?.unverified(/*used as a bitfield, this is safe*/);

        let _protocol: u8 = decoder
            .read_u8()?
            .verify_unwrap(|protocol| {
                // RFC 4034                DNSSEC Resource Records               March 2005
                //
                // 2.1.2.  The Protocol Field
                //
                //    The Protocol Field MUST have value 3, and the DNSKEY RR MUST be
                //    treated as invalid during signature verification if it is found to be
                //    some value other than 3.
                //
                // protocol is defined to only be '3' right now

                *protocol == 3
            })
            .map_err(|protocol| ProtoError::from(ProtoErrorKind::DnsKeyProtocolNot3(protocol)))?;

        let algorithm: Algorithm = Algorithm::read(decoder)?;

        // the public key is the left-over bytes minus 4 for the first fields
        //   this sub is safe, as the first 4 fields must have been in the rdata, otherwise there would have been
        //   an earlier return.
        let key_len = length
        .map(|u| u as usize)
        .checked_sub(4)
        .map_err(|_| ProtoError::from("invalid rdata length in DNSKEY"))?
        .unverified(/*used only as length safely*/);
        let public_key =
            decoder.read_vec(key_len)?.unverified(/*the byte array will fail in usage if invalid*/);

        Ok(Self::with_flags(
            flags,
            PublicKeyBuf::new(public_key, algorithm),
        ))
    }
}

impl RecordData for DNSKEY {
    fn try_borrow(data: &RData) -> Option<&Self> {
        match data {
            RData::DNSSEC(DNSSECRData::DNSKEY(csync)) => Some(csync),
            _ => None,
        }
    }

    fn record_type(&self) -> RecordType {
        RecordType::DNSKEY
    }

    fn into_rdata(self) -> RData {
        RData::DNSSEC(DNSSECRData::DNSKEY(self))
    }
}

impl Verifier for DNSKEY {
    fn algorithm(&self) -> Algorithm {
        self.public_key.algorithm()
    }

    fn key(&self) -> ProtoResult<Arc<dyn PublicKey + '_>> {
        decode_public_key(self.public_key.public_bytes(), self.public_key.algorithm())
    }
}

/// [RFC 4034, DNSSEC Resource Records, March 2005](https://tools.ietf.org/html/rfc4034#section-2.2)
///
/// ```text
/// 2.2.  The DNSKEY RR Presentation Format
///
///    The presentation format of the RDATA portion is as follows:
///
///    The Flag field MUST be represented as an unsigned decimal integer.
///    Given the currently defined flags, the possible values are: 0, 256,
///    and 257.
///
///    The Protocol Field MUST be represented as an unsigned decimal integer
///    with a value of 3.
///
///    The Algorithm field MUST be represented either as an unsigned decimal
///    integer or as an algorithm mnemonic as specified in Appendix A.1.
///
///    The Public Key field MUST be represented as a Base64 encoding of the
///    Public Key.  Whitespace is allowed within the Base64 text.  For a
///    definition of Base64 encoding, see [RFC3548].
///
/// 2.3.  DNSKEY RR Example
///
///    The following DNSKEY RR stores a DNS zone key for example.com.
///
///    example.com. 86400 IN DNSKEY 256 3 5 ( AQPSKmynfzW4kyBv015MUG2DeIQ3
///                                           Cbl+BBZH4b/0PY1kxkmvHjcZc8no
///                                           kfzj31GajIQKY+5CptLr3buXA10h
///                                           WqTkF7H6RfoRqXQeogmMHfpftf6z
///                                           Mv1LyBUgia7za6ZEzOJBOztyvhjL
///                                           742iU/TpPSEDhm2SNKLijfUppn1U
///                                           aNvv4w==  )
///
///    The first four text fields specify the owner name, TTL, Class, and RR
///    type (DNSKEY).  Value 256 indicates that the Zone Key bit (bit 7) in
///    the Flags field has value 1.  Value 3 is the fixed Protocol value.
///    Value 5 indicates the public key algorithm.  Appendix A.1 identifies
///    algorithm type 5 as RSA/SHA1 and indicates that the format of the
///    RSA/SHA1 public key field is defined in [RFC3110].  The remaining
///    text is a Base64 encoding of the public key.
/// ```
impl fmt::Display for DNSKEY {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "{flags} 3 {alg} {key}",
            flags = self.flags(),
            alg = u8::from(self.public_key.algorithm()),
            key = data_encoding::BASE64.encode(self.public_key.public_bytes())
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::dbg_macro, clippy::print_stdout)]

    #[cfg(feature = "std")]
    use std::println;

    use rustls_pki_types::PrivateKeyDer;

    use super::*;
    use crate::dnssec::{SigningKey, crypto::EcdsaSigningKey};

    #[test]
    fn test() {
        let algorithm = Algorithm::ECDSAP256SHA256;
        let pkcs8 = EcdsaSigningKey::generate_pkcs8(algorithm).unwrap();
        let signing_key =
            EcdsaSigningKey::from_key_der(&PrivateKeyDer::from(pkcs8), algorithm).unwrap();

        let rdata = DNSKEY::new(
            true,
            true,
            false,
            PublicKeyBuf::new(
                signing_key
                    .to_public_key()
                    .unwrap()
                    .public_bytes()
                    .to_owned(),
                algorithm,
            ),
        );

        let mut bytes = Vec::new();
        let mut encoder: BinEncoder<'_> = BinEncoder::new(&mut bytes);
        assert!(rdata.emit(&mut encoder).is_ok());
        let bytes = encoder.into_bytes();

        #[cfg(feature = "std")]
        println!("bytes: {bytes:?}");

        let mut decoder: BinDecoder<'_> = BinDecoder::new(bytes);
        let read_rdata = DNSKEY::read_data(&mut decoder, Restrict::new(bytes.len() as u16));
        let read_rdata = read_rdata.expect("error decoding");

        assert_eq!(rdata, read_rdata);
        assert!(
            rdata
                .to_digest(
                    &Name::parse("www.example.com.", None).unwrap(),
                    DigestType::SHA256
                )
                .is_ok()
        );
    }

    #[test]
    fn test_reserved_flags() {
        let rdata =
            DNSKEY::with_flags(u16::MAX, PublicKeyBuf::new(vec![0u8], Algorithm::RSASHA256));

        let mut bytes = Vec::new();
        let mut encoder = BinEncoder::new(&mut bytes);
        rdata.emit(&mut encoder).expect("error encoding");
        let bytes = encoder.into_bytes();

        println!("bytes: {bytes:?}");

        let mut decoder = BinDecoder::new(bytes);
        let read_rdata = DNSKEY::read_data(&mut decoder, Restrict::new(bytes.len() as u16))
            .expect("error decoding");

        assert_eq!(rdata, read_rdata);
    }

    #[test]
    fn test_calculate_key_tag_checksum() {
        let test_text = "The quick brown fox jumps over the lazy dog";
        let test_vectors = vec![
            (vec![], 0),
            (vec![0, 0, 0, 0], 0),
            (vec![0xff, 0xff, 0xff, 0xff], 0xffff),
            (vec![1, 0, 0, 0], 0x0100),
            (vec![0, 1, 0, 0], 0x0001),
            (vec![0, 0, 1, 0], 0x0100),
            (test_text.as_bytes().to_vec(), 0x8d5b),
        ];

        for (input_data, exp_result) in test_vectors {
            let result = DNSKEY::calculate_key_tag_internal(&input_data);
            assert_eq!(result, exp_result);
        }
    }
}
