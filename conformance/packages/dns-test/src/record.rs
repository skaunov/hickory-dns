//! Text representation of DNS records

use core::result::Result as CoreResult;
use core::str::FromStr;
use core::{array, fmt};
use std::borrow::Cow;
use std::fmt::Write;
use std::net::Ipv4Addr;
use std::{any, mem};

use crate::{DEFAULT_TTL, Error, FQDN, Result};

const CLASS: &str = "IN"; // "internet"

macro_rules! record_types {
    ($($variant:ident),*) => {
        #[allow(clippy::upper_case_acronyms)]
        #[derive(Debug, PartialEq, Clone)]
        pub enum RecordType {
            $($variant),*,
            Unknown(u16),
        }

        impl RecordType {
            pub fn as_name(&self) -> Cow<'static, str> {
                match self {
                    $(Self::$variant => Cow::Borrowed(stringify!($variant))),*,
                    Self::Unknown(code) => Cow::Owned(format!("type{code}")),
                }
            }
        }

        impl FromStr for RecordType {
            type Err = Error;

            fn from_str(input: &str) -> Result<Self> {
                $(if input == stringify!($variant) {
                    return Ok(Self::$variant);
                })*

                let lowercase = input.to_lowercase();
                if let Some(type_code_str) = lowercase.strip_prefix("type") {
                    if let Ok(type_code) = type_code_str.parse() {
                        return Ok(Self::Unknown(type_code))
                    }
                }

                Err(format!("unknown record type: {input}").into())
            }
        }

        impl fmt::Display for RecordType {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_name().as_ref())
            }
        }
    };
}

record_types!(
    A, AAAA, CAA, CNAME, DNSKEY, DS, MX, NS, NSEC, NSEC3, NSEC3PARAM, RRSIG, SOA, TXT
);

#[derive(Debug, Clone)]
#[allow(clippy::upper_case_acronyms)]
pub enum Record {
    A(A),
    CAA(CAA),
    CNAME(CNAME),
    DNSKEY(DNSKEY),
    DS(DS),
    NS(NS),
    NSEC(NSEC),
    NSEC3(NSEC3),
    NSEC3PARAM(NSEC3PARAM),
    RRSIG(RRSIG),
    SOA(SOA),
    TXT(TXT),
    Unknown(UnknownRdata),
}

impl From<NSEC3> for Record {
    fn from(v: NSEC3) -> Self {
        Self::NSEC3(v)
    }
}

impl From<DNSKEY> for Record {
    fn from(v: DNSKEY) -> Self {
        Self::DNSKEY(v)
    }
}

impl From<DS> for Record {
    fn from(v: DS) -> Self {
        Self::DS(v)
    }
}

impl From<A> for Record {
    fn from(v: A) -> Self {
        Self::A(v)
    }
}

impl From<CNAME> for Record {
    fn from(v: CNAME) -> Self {
        Self::CNAME(v)
    }
}

impl From<NS> for Record {
    fn from(v: NS) -> Self {
        Self::NS(v)
    }
}

impl From<RRSIG> for Record {
    fn from(v: RRSIG) -> Self {
        Self::RRSIG(v)
    }
}

impl From<SOA> for Record {
    fn from(v: SOA) -> Self {
        Self::SOA(v)
    }
}

impl Record {
    pub fn as_rrsig_mut(&mut self) -> Option<&mut RRSIG> {
        if let Self::RRSIG(rrsig) = self {
            Some(rrsig)
        } else {
            None
        }
    }

    pub fn try_into_a(self) -> CoreResult<A, Self> {
        if let Self::A(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn try_into_cname(self) -> CoreResult<CNAME, Self> {
        if let Self::CNAME(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn try_into_rrsig(self) -> CoreResult<RRSIG, Self> {
        if let Self::RRSIG(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn try_into_soa(self) -> CoreResult<SOA, Self> {
        if let Self::SOA(soa) = self {
            Ok(soa)
        } else {
            Err(self)
        }
    }

    pub fn try_into_txt(self) -> CoreResult<TXT, Self> {
        if let Self::TXT(txt) = self {
            Ok(txt)
        } else {
            Err(self)
        }
    }

    pub fn is_soa(&self) -> bool {
        matches!(self, Self::SOA(..))
    }

    pub fn a(fqdn: FQDN, ipv4_addr: Ipv4Addr) -> Self {
        A {
            fqdn,
            ttl: DEFAULT_TTL,
            ipv4_addr,
        }
        .into()
    }

    pub fn cname(fqdn: FQDN, target: FQDN) -> Self {
        CNAME {
            fqdn,
            target,
            ttl: DEFAULT_TTL,
        }
        .into()
    }

    pub fn ns(zone: FQDN, nameserver: FQDN) -> Self {
        NS {
            zone,
            ttl: DEFAULT_TTL,
            nameserver,
        }
        .into()
    }

    pub fn try_into_ds(self) -> CoreResult<DS, Self> {
        if let Self::DS(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn try_into_nsec3(self) -> CoreResult<NSEC3, Self> {
        if let Self::NSEC3(v) = self {
            Ok(v)
        } else {
            Err(self)
        }
    }

    pub fn try_into_caa(self) -> CoreResult<CAA, Self> {
        match self {
            Self::CAA(v) => Ok(v),
            _ => Err(self),
        }
    }
}

impl FromStr for Record {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let record_type = input
            .split_whitespace()
            .nth(3)
            .ok_or("record is missing the type column")?;

        let record = match record_type {
            "A" => Record::A(input.parse()?),
            "CAA" => Record::CAA(input.parse()?),
            "CNAME" => Record::CNAME(input.parse()?),
            "DNSKEY" => Record::DNSKEY(input.parse()?),
            "DS" => Record::DS(input.parse()?),
            "NS" => Record::NS(input.parse()?),
            "NSEC" => Record::NSEC(input.parse()?),
            "NSEC3" => Record::NSEC3(input.parse()?),
            "NSEC3PARAM" => Record::NSEC3PARAM(input.parse()?),
            "RRSIG" => Record::RRSIG(input.parse()?),
            "SOA" => Record::SOA(input.parse()?),
            "TXT" => Record::TXT(input.parse()?),
            _ => {
                if record_type.starts_with("TYPE") {
                    Record::Unknown(input.parse()?)
                } else {
                    return Err(format!("unknown record type: {record_type}").into());
                }
            }
        };

        Ok(record)
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Record::A(a) => write!(f, "{a}"),
            Record::CAA(caa) => write!(f, "{caa}"),
            Record::CNAME(cname) => write!(f, "{cname}"),
            Record::DS(ds) => write!(f, "{ds}"),
            Record::DNSKEY(dnskey) => write!(f, "{dnskey}"),
            Record::NS(ns) => write!(f, "{ns}"),
            Record::NSEC(nsec) => write!(f, "{nsec}"),
            Record::NSEC3(nsec3) => write!(f, "{nsec3}"),
            Record::NSEC3PARAM(nsec3param) => write!(f, "{nsec3param}"),
            Record::RRSIG(rrsig) => write!(f, "{rrsig}"),
            Record::SOA(soa) => write!(f, "{soa}"),
            Record::TXT(txt) => write!(f, "{txt}"),
            Record::Unknown(other) => write!(f, "{other}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct A {
    pub fqdn: FQDN,
    pub ttl: u32,
    pub ipv4_addr: Ipv4Addr,
}

impl FromStr for A {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(fqdn),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(ipv4_addr),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 5 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        Ok(Self {
            fqdn: fqdn.parse()?,
            ttl: ttl.parse()?,
            ipv4_addr: ipv4_addr.parse()?,
        })
    }
}

impl fmt::Display for A {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            fqdn,
            ttl,
            ipv4_addr,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{fqdn}\t{ttl}\t{CLASS}\t{record_type}\t{ipv4_addr}")
    }
}

#[derive(Debug, Clone)]
pub struct CNAME {
    pub fqdn: FQDN,
    pub ttl: u32,
    pub target: FQDN,
}

impl FromStr for CNAME {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(fqdn),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(target),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 5 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        Ok(Self {
            fqdn: fqdn.parse()?,
            ttl: ttl.parse()?,
            target: target.parse()?,
        })
    }
}

impl fmt::Display for CNAME {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { fqdn, ttl, target } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{fqdn}\t{ttl}\t{CLASS}\t{record_type}\t{target}")
    }
}

// integer types chosen based on bit sizes in section 2.1 of RFC4034
#[derive(Clone, Debug)]
pub struct DNSKEY {
    pub zone: FQDN,
    pub ttl: u32,
    pub rdata: DNSKEYRData,
}

impl DNSKEY {
    const KSK_BIT: u16 = 1;

    /// formats the `DNSKEY` in the format `delv` expects
    pub(super) fn delv(&self) -> String {
        let Self {
            zone,
            rdata:
                DNSKEYRData {
                    flags,
                    protocol,
                    algorithm,
                    public_key,
                },
            ..
        } = self;

        format!("{zone} static-key {flags} {protocol} {algorithm} \"{public_key}\";\n")
    }

    pub fn clear_key_signing_key_bit(&mut self) {
        self.rdata.flags &= !Self::KSK_BIT;
    }

    pub fn is_key_signing_key(&self) -> bool {
        let mask = Self::KSK_BIT;
        self.rdata.flags & mask == mask
    }

    pub fn is_zone_signing_key(&self) -> bool {
        !self.is_key_signing_key()
    }
}

impl FromStr for DNSKEY {
    type Err = Error;

    fn from_str(mut input: &str) -> Result<Self> {
        if let Some((rr, _comment)) = input.rsplit_once(" ;") {
            input = rr.trim_end();
        }

        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(flags),
            Some(protocol),
            Some(algorithm),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 7 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut public_key = String::new();
        for column in columns {
            public_key.push_str(column);
        }

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            rdata: DNSKEYRData {
                flags: flags.parse()?,
                protocol: protocol.parse()?,
                algorithm: algorithm.parse()?,
                public_key,
            },
        })
    }
}

impl fmt::Display for DNSKEY {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            rdata:
                DNSKEYRData {
                    flags,
                    protocol,
                    algorithm,
                    public_key,
                },
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{flags} {protocol} {algorithm}"
        )?;

        write_split_long_string(f, public_key)
    }
}

#[derive(Clone, Debug)]
pub struct DNSKEYRData {
    pub flags: u16,
    pub protocol: u8,
    pub algorithm: u8,
    pub public_key: String,
}

impl DNSKEYRData {
    // as per appendix B of RFC4034
    pub fn calculate_key_tag(&self) -> u16 {
        use base64::prelude::*;

        let public_key = &BASE64_STANDARD
            .decode(self.public_key.as_bytes())
            .expect("base64 decoding failed");

        if self.algorithm == 1 {
            let len = public_key.len();
            let start = len - 3;
            return u16::from_be_bytes(public_key[start..start + 2].try_into().unwrap());
        }

        let mut rdata = Vec::<u8>::new();
        rdata.extend_from_slice(self.flags.to_be_bytes().as_slice());
        rdata.push(3); // protocol
        rdata.push(self.algorithm);
        rdata.extend_from_slice(public_key);

        let mut acc = 0u32;
        for chunk in rdata.chunks(2) {
            let halfword = if let Ok(array) = chunk.try_into() {
                u16::from_be_bytes(array)
            } else {
                u16::from(chunk[0]) << 8
            };
            acc += u32::from(halfword);
        }
        acc += acc >> 16;

        acc as u16
    }
}

#[derive(Clone, Debug)]
pub struct DS {
    pub zone: FQDN,
    pub ttl: u32,
    pub key_tag: u16,
    pub algorithm: u8,
    pub digest_type: u8,
    pub digest: String,
}

impl FromStr for DS {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(key_tag),
            Some(algorithm),
            Some(digest_type),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 7 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut digest = String::new();
        for column in columns {
            digest.push_str(column);
        }

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            key_tag: key_tag.parse()?,
            algorithm: algorithm.parse()?,
            digest_type: digest_type.parse()?,
            digest,
        })
    }
}

impl fmt::Display for DS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            key_tag,
            algorithm,
            digest_type,
            digest,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{key_tag} {algorithm} {digest_type}"
        )?;

        write_split_long_string(f, digest)
    }
}

#[derive(Debug, Clone)]
pub struct NS {
    pub zone: FQDN,
    pub ttl: u32,
    pub nameserver: FQDN,
}

impl fmt::Display for NS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            nameserver,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{nameserver}")
    }
}

impl FromStr for NS {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(nameserver),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 5 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            nameserver: nameserver.parse()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NSEC {
    pub fqdn: FQDN,
    pub ttl: u32,
    pub next_domain: FQDN,
    pub record_types: Vec<RecordType>,
}

impl FromStr for NSEC {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(fqdn),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(next_domain),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 5 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut record_types = vec![];
        for column in columns {
            record_types.push(column.parse()?);
        }

        Ok(Self {
            fqdn: fqdn.parse()?,
            ttl: ttl.parse()?,
            next_domain: next_domain.parse()?,
            record_types,
        })
    }
}

impl fmt::Display for NSEC {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            fqdn,
            ttl,
            next_domain,
            record_types,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{fqdn}\t{ttl}\t{CLASS}\t{record_type}\t{next_domain}")?;

        for record_type in record_types {
            write!(f, " {record_type}")?;
        }

        Ok(())
    }
}

// integer types chosen based on bit sizes in section 3.2 of RFC5155
#[derive(Debug, Clone, PartialEq)]
pub struct NSEC3 {
    pub fqdn: FQDN,
    pub ttl: u32,
    pub hash_alg: u8,
    pub flags: u8,
    pub iterations: u16,
    pub salt: String,
    pub next_hashed_owner_name: String,
    pub record_types: Vec<RecordType>,
}

impl FromStr for NSEC3 {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(fqdn),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(hash_alg),
            Some(flags),
            Some(iterations),
            Some(salt),
            Some(next_hashed_owner_name),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 9 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut record_types = vec![];
        for column in columns {
            record_types.push(column.parse()?);
        }

        Ok(Self {
            fqdn: fqdn.parse()?,
            ttl: ttl.parse()?,
            hash_alg: hash_alg.parse()?,
            flags: flags.parse()?,
            iterations: iterations.parse()?,
            salt: salt.to_string(),
            next_hashed_owner_name: next_hashed_owner_name.to_string(),
            record_types,
        })
    }
}

impl fmt::Display for NSEC3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            fqdn,
            ttl,
            hash_alg,
            flags,
            iterations,
            salt,
            next_hashed_owner_name,
            record_types,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{fqdn}\t{ttl}\t{CLASS}\t{record_type}\t{hash_alg} {flags} {iterations} {salt}  {next_hashed_owner_name}"
        )?;

        for record_type in record_types {
            write!(f, " {record_type}")?;
        }

        Ok(())
    }
}

// integer types chosen based on bit sizes in section 4.2 of RFC5155
#[derive(Debug, Clone)]
pub struct NSEC3PARAM {
    pub zone: FQDN,
    pub ttl: u32,
    pub hash_alg: u8,
    pub flags: u8,
    pub iterations: u16,
}

impl FromStr for NSEC3PARAM {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(hash_alg),
            Some(flags),
            Some(iterations),
            Some(dash),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 8 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        if dash != "-" {
            todo!("salt is not implemented")
        }

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            hash_alg: hash_alg.parse()?,
            flags: flags.parse()?,
            iterations: iterations.parse()?,
        })
    }
}

impl fmt::Display for NSEC3PARAM {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            hash_alg,
            flags,
            iterations,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{hash_alg} {flags} {iterations} -"
        )
    }
}

// integer types chosen based on bit sizes in section 3.1 of RFC4034
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone)]
pub struct RRSIG {
    pub fqdn: FQDN,
    pub ttl: u32,
    pub type_covered: RecordType,
    pub algorithm: u8,
    pub labels: u8,
    pub original_ttl: u32,
    // NOTE on the wire these are 32-bit UNIX timestamps but in text representation they are
    // `strftime` formatted
    // TODO switch these to `chrono::DateTime<Utc>`?
    pub signature_expiration: u64,
    pub signature_inception: u64,
    pub key_tag: u16,
    pub signer_name: FQDN,
    /// base64 encoded
    pub signature: String,
}

impl FromStr for RRSIG {
    type Err = Error;

    fn from_str(input: &str) -> CoreResult<Self, Self::Err> {
        let mut columns = input.split_whitespace();

        let [
            Some(fqdn),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(type_covered),
            Some(algorithm),
            Some(labels),
            Some(original_ttl),
            Some(signature_expiration),
            Some(signature_inception),
            Some(key_tag),
            Some(signer_name),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 12 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut signature = String::new();
        for column in columns {
            signature.push_str(column);
        }

        Ok(Self {
            fqdn: fqdn.parse()?,
            ttl: ttl.parse()?,
            type_covered: type_covered.parse()?,
            algorithm: algorithm.parse()?,
            labels: labels.parse()?,
            original_ttl: original_ttl.parse()?,
            signature_expiration: signature_expiration.parse()?,
            signature_inception: signature_inception.parse()?,
            key_tag: key_tag.parse()?,
            signer_name: signer_name.parse()?,
            signature,
        })
    }
}

impl fmt::Display for RRSIG {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            fqdn,
            ttl,
            type_covered,
            algorithm,
            labels,
            original_ttl,
            signature_expiration,
            signature_inception,
            key_tag,
            signer_name,
            signature,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{fqdn}\t{ttl}\t{CLASS}\t{record_type}\t{type_covered} {algorithm} {labels} {original_ttl} {signature_expiration} {signature_inception} {key_tag} {signer_name}"
        )?;

        write_split_long_string(f, signature)
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone)]
pub struct SOA {
    pub zone: FQDN,
    pub ttl: u32,
    pub nameserver: FQDN,
    pub admin: FQDN,
    pub settings: SoaSettings,
}

impl FromStr for SOA {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(nameserver),
            Some(admin),
            Some(serial),
            Some(refresh),
            Some(retry),
            Some(expire),
            Some(minimum),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 11 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            nameserver: nameserver.parse()?,
            admin: admin.parse()?,
            settings: SoaSettings {
                serial: serial.parse()?,
                refresh: refresh.parse()?,
                retry: retry.parse()?,
                expire: expire.parse()?,
                minimum: minimum.parse()?,
            },
        })
    }
}

impl fmt::Display for SOA {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            nameserver,
            admin,
            settings,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(
            f,
            "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{nameserver} {admin} {settings}"
        )
    }
}

#[derive(Debug, Clone)]
pub struct SoaSettings {
    pub serial: u32,
    pub refresh: u32,
    pub retry: u32,
    pub expire: u32,
    pub minimum: u32,
}

impl Default for SoaSettings {
    fn default() -> Self {
        Self {
            serial: 2024010101,
            refresh: 1800,  // 30 minutes
            retry: 900,     // 15 minutes
            expire: 604800, // 1 week
            minimum: 86400, // 1 day
        }
    }
}

impl fmt::Display for SoaSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            serial,
            refresh,
            retry,
            expire,
            minimum,
        } = self;

        write!(f, "{serial} {refresh} {retry} {expire} {minimum}")
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone)]
pub struct TXT {
    pub zone: FQDN,
    pub ttl: u32,
    pub character_strings: Vec<String>,
}

impl FromStr for TXT {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self> {
        let mut rest = input;
        let [Some(zone), Some(ttl), Some(class), Some(record_type)] = array::from_fn(|_| {
            if let Some((left, right)) = rest.split_once(|c| char::is_ascii_whitespace(&c)) {
                rest = right.trim();
                Some(left)
            } else {
                let trimmed = rest.trim();
                rest = "";
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            }
        }) else {
            return Err("expected at least 5 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let mut character_strings = Vec::new();
        let mut current_string = String::new();

        enum State {
            /// At the start of the input, or after a string.
            Whitespace,
            /// In an unquoted string.
            UnquotedString,
            /// In a quoted string.
            QuotedString,
        }

        let mut state = State::Whitespace;
        for character in rest.chars() {
            if !character.is_ascii() {
                return Err("non-ASCII characters in TXT records are not supported".into());
            }
            match (state, character) {
                (State::Whitespace, character) if character.is_ascii_whitespace() => {
                    state = State::Whitespace;
                }
                (State::Whitespace, '"') => {
                    state = State::QuotedString;
                }
                (State::UnquotedString, character) if character.is_ascii_whitespace() => {
                    character_strings.push(mem::take(&mut current_string));
                    state = State::Whitespace;
                }
                (State::QuotedString, '"') => {
                    character_strings.push(mem::take(&mut current_string));
                    state = State::Whitespace;
                }
                (State::Whitespace, '(') => {
                    return Err("multi-line TXT records are not supported".into());
                }
                (_, '@') => {
                    return Err(
                        "denoting the current origin with @ in TXT records is not supported".into(),
                    );
                }
                (_, '\\') => {
                    return Err("backslash escapes in TXT records are not supported".into());
                }
                (State::Whitespace | State::UnquotedString, character) => {
                    current_string.push(character);
                    state = State::UnquotedString;
                }
                (State::QuotedString, character) => {
                    current_string.push(character);
                    state = State::QuotedString;
                }
            }
        }
        match state {
            State::Whitespace => {}
            State::UnquotedString => character_strings.push(mem::take(&mut current_string)),
            State::QuotedString => return Err("quoted string in TXT record was not closed".into()),
        }

        if character_strings.is_empty() {
            return Err("expected at least 5 columns".into());
        }

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            character_strings,
        })
    }
}

impl fmt::Display for TXT {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            character_strings,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{zone}\t{ttl}\t{CLASS}\t{record_type}")?;
        let mut is_first = true;
        for string in character_strings.iter() {
            if is_first {
                write!(f, "\t\"{string}\"")?;
                is_first = false;
            } else {
                write!(f, " \"{string}\"")?;
            }
        }
        Ok(())
    }
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone)]
pub struct CAA {
    pub zone: FQDN,
    pub ttl: u32,
    pub flags: u8,
    pub tag: String,
    pub value: String,
}

impl FromStr for CAA {
    type Err = Error;

    fn from_str(input: &str) -> CoreResult<Self, Self::Err> {
        let mut columns = input.split_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(flags),
            Some(tag),
            Some(value),
            None,
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected 7 columns".into());
        };

        check_record_type::<Self>(record_type)?;
        check_class(class)?;

        let value = if value == "\"\"" {
            "".to_string()
        } else {
            value.to_string()
        };

        Ok(Self {
            zone: zone.parse()?,
            ttl: ttl.parse()?,
            flags: flags.parse()?,
            tag: tag.to_string(),
            value,
        })
    }
}

impl fmt::Display for CAA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            zone,
            ttl,
            flags,
            tag,
            value,
        } = self;

        let record_type = unqualified_type_name::<Self>();
        write!(f, "{zone}\t{ttl}\t{CLASS}\t{record_type}\t{flags} {tag} ")?;
        if value.is_empty() {
            write!(f, "\"\"")?;
        } else {
            f.write_str(value)?;
        }

        Ok(())
    }
}

/// A record of unknown type.
#[derive(Debug, Clone)]
pub struct UnknownRdata {
    pub zone: FQDN,
    pub ttl: u32,
    pub r#type: u16,
    pub rdata: Vec<u8>,
}

impl FromStr for UnknownRdata {
    type Err = Error;

    fn from_str(input: &str) -> CoreResult<Self, Self::Err> {
        let mut columns = input.split_ascii_whitespace();

        let [
            Some(zone),
            Some(ttl),
            Some(class),
            Some(record_type),
            Some(generic_encoding_token),
            Some(rdata_length),
        ] = array::from_fn(|_| columns.next())
        else {
            return Err("expected at least 6 columns".into());
        };

        check_class(class)?;
        let Some(type_number) = record_type.strip_prefix("TYPE") else {
            return Err(
                "tried to parse `{record_type}` record as a generic unknown type record".into(),
            );
        };
        let r#type = type_number.parse()?;

        if generic_encoding_token != "\\#" {
            return Err("tried to parse a record of unknown type but \\# was not present".into());
        }

        let mut rdata = vec![];
        for column in columns {
            rdata.extend(hex::decode(column)?);
        }
        if rdata.len() != rdata_length.parse::<usize>()? {
            return Err("inconsistent RDATA length".into());
        }

        Ok({
            Self {
                zone: zone.parse()?,
                ttl: ttl.parse()?,
                r#type,
                rdata,
            }
        })
    }
}

impl fmt::Display for UnknownRdata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            zone,
            ttl,
            r#type,
            rdata,
        } = self;

        write!(f, "{zone}\t{ttl}\t{CLASS}\tTYPE{type}\t\\# {}", rdata.len())?;
        for byte in rdata {
            write!(f, " {byte:02x}")?;
        }
        Ok(())
    }
}

fn check_class(class: &str) -> Result<()> {
    if class != "IN" {
        return Err(format!("unknown class: {class}").into());
    }

    Ok(())
}

fn check_record_type<T>(record_type: &str) -> Result<()> {
    let expected = unqualified_type_name::<T>();
    if record_type == expected {
        Ok(())
    } else {
        Err(format!("tried to parse `{record_type}` record as an {expected} record").into())
    }
}

fn unqualified_type_name<T>() -> &'static str {
    let name = any::type_name::<T>();
    if let Some((_rest, component)) = name.rsplit_once(':') {
        component
    } else {
        name
    }
}

pub(crate) fn write_split_long_string(f: &mut fmt::Formatter<'_>, field: &str) -> fmt::Result {
    for (index, c) in field.chars().enumerate() {
        if index % 56 == 0 {
            f.write_char(' ')?;
        }
        f.write_char(c)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;

    // dig A a.root-servers.net
    const A_INPUT: &str = "a.root-servers.net.	77859	IN	A	198.41.0.4";

    #[test]
    fn a() -> Result<()> {
        let a @ A {
            fqdn,
            ttl,
            ipv4_addr,
        } = &A_INPUT.parse()?;

        assert_eq!("a.root-servers.net.", fqdn.as_str());
        assert_eq!(77859, *ttl);
        assert_eq!(Ipv4Addr::new(198, 41, 0, 4), *ipv4_addr);

        let output = a.to_string();
        assert_eq!(A_INPUT, output);

        Ok(())
    }

    // dig CNAME www.isc.org
    const CNAME_INPUT: &str = "www.isc.org.	277	IN	CNAME	isc.map.fastlydns.net.";

    #[test]
    fn cname() -> Result<()> {
        let cname @ CNAME { fqdn, ttl, target } = &CNAME_INPUT.parse()?;

        assert_eq!("www.isc.org.", fqdn.as_str());
        assert_eq!(277, *ttl);
        assert_eq!("isc.map.fastlydns.net.", target.as_str());

        let output = cname.to_string();
        assert_eq!(CNAME_INPUT, output);

        Ok(())
    }

    // dig DNSKEY .
    const DNSKEY_INPUT: &str = ".	1116	IN	DNSKEY	257 3 8 AwEAAaz/tAm8yTn4Mfeh5eyI96WSVexTBAvkMgJzkKTOiW1vkIbzxeF3 +/4RgWOq7HrxRixHlFlExOLAJr5emLvN7SWXgnLh4+B5xQlNVz8Og8kv ArMtNROxVQuCaSnIDdD5LKyWbRd2n9WGe2R8PzgCmr3EgVLrjyBxWezF 0jLHwVN8efS3rCj/EWgvIWgb9tarpVUDK/b58Da+sqqls3eNbuv7pr+e oZG+SrDK6nWeL3c6H5Apxz7LjVc1uTIdsIXxuOLYA4/ilBmSVIzuDWfd RUfhHdY6+cn8HFRm+2hM8AnXGXws9555KrUB5qihylGa8subX2Nn6UwN R1AkUTV74bU=";

    #[test]
    fn dnskey() -> Result<()> {
        let dnskey @ DNSKEY {
            zone,
            ttl,
            rdata:
                DNSKEYRData {
                    flags,
                    protocol,
                    algorithm,
                    public_key,
                },
        } = &DNSKEY_INPUT.parse()?;

        assert_eq!(FQDN::ROOT, *zone);
        assert_eq!(1116, *ttl);
        assert_eq!(257, *flags);
        assert_eq!(3, *protocol);
        assert_eq!(8, *algorithm);
        let expected = "AwEAAaz/tAm8yTn4Mfeh5eyI96WSVexTBAvkMgJzkKTOiW1vkIbzxeF3+/4RgWOq7HrxRixHlFlExOLAJr5emLvN7SWXgnLh4+B5xQlNVz8Og8kvArMtNROxVQuCaSnIDdD5LKyWbRd2n9WGe2R8PzgCmr3EgVLrjyBxWezF0jLHwVN8efS3rCj/EWgvIWgb9tarpVUDK/b58Da+sqqls3eNbuv7pr+eoZG+SrDK6nWeL3c6H5Apxz7LjVc1uTIdsIXxuOLYA4/ilBmSVIzuDWfdRUfhHdY6+cn8HFRm+2hM8AnXGXws9555KrUB5qihylGa8subX2Nn6UwNR1AkUTV74bU=";
        assert_eq!(expected, public_key);
        // `dig +multi DNSKEY .`
        assert_eq!(20326, dnskey.rdata.calculate_key_tag());

        let output = dnskey.to_string();
        assert_eq!(DNSKEY_INPUT, output);

        Ok(())
    }

    #[test]
    fn rsamd5_key_tag() -> Result<()> {
        // dig @1.1.1.1 +recurse +cdflag DNSKEY rsamd5.extended-dns-errors.com.
        const INPUT: &str = "rsamd5.extended-dns-errors.com.    268 IN DNSKEY 257 3 1 \
                             AwEAAcpRn4ct2tt2a6RRqOYEDMtK8zETcLvpSoHhthWF \
                             8WBvko0XodJJYlstLN6JMb5NwRAgcfddH3sR/ELdw2Hk \
                             Hrp/jRRBW4wAHiVZU1uDRml0pD9ZEWQ3It+eDp/lG+Cp \
                             Q3e5BGibTPCoWtOvx5uZQDkLlQBAXu2vTn1w2VXCMuwP";

        let dnskey: DNSKEY = INPUT.parse()?;

        // dig @1.1.1.1 +recurse +cdflag DS rsamd5.extended-dns-errors.com.
        // NB `dig +multi` (dnsutils 9.18.28) reports a wrong key tag
        assert_eq!(13036, dnskey.rdata.calculate_key_tag());

        Ok(())
    }

    #[test]
    fn dsa_key_tag() -> Result<()> {
        // dig @1.1.1.1 +recurse +cdflag +multi DNSKEY rsamd5.extended-dns-errors.com.
        const INPUT: &str = "dsa.extended-dns-errors.com. 600 IN DNSKEY 257 3 3 \
                             CPn4bkeyFLewxmOnFPoNLE1dTSHh/sDgPPPtKvXgtp9N \
                             3r8Z4TEOKzrtmTf/+7o09GFQPApzqabgU2Wx3mxbgawJ \
                             jbu1JEp1nkYuu0tdHcndLKnPjJpkQx5kOZveQ8ggxUiX \
                             iq9Q8plA3n/k8UZXmnf1BC3kb9FEDH7v9SIrn7tyE1Z+ \
                             H5MXxw5Z+Qn4n34v3Z/L7UdQ9+aY70l5Maip0fhQksGZ \
                             CbPqZeuKGvUIA4HR70HGvrrXZ/Bti/h1i92JCZRxnKrq \
                             rBKN7tz6L2iWRrAuOnznbuc+yKbBQC+hw0a9SqB3cxFz \
                             CjXekaZ5XX99c+QZCXQdqTa44bE2WsLbSgv4JfCeLzLS \
                             5ChYMHvqnCoe33wC6yvB967+9OxPoH5Ctndtc8SOW2bE \
                             dI4njhw/27k9Z5zN6B/QgIFEPwY3nHkduNif3ugOHmEY \
                             IefHyDDZq8NcZfFVM7Hu27utxxZhv2NkGsFLqKMuUi5i \
                             AX7BO7BglvRnPRq31BQ0vpiw8vJzAbx6p+jdKXvpa1tG \
                             zG4m3iwDQacz";

        let dnskey: DNSKEY = INPUT.parse()?;

        // from `dig +multi`
        assert_eq!(49845, dnskey.rdata.calculate_key_tag());

        Ok(())
    }

    #[test]
    fn parsing_dnskey_ignores_trailing_comment() -> Result<()> {
        // `ldns-signzone`'s output
        const DNSKEY_INPUT2: &str = ".	86400	IN	DNSKEY	256 3 7 AwEAAbEzD/uB2WK89f+PJ1Lyg5xvdt9mXge/R5tiQl8SEAUh/kfbn8jQiakH3HbBnBtdNXpjYrsmM7AxMmJLrp75dFMVnl5693/cY5k4dSk0BFJPQtBsZDn/7Q1rviQn0gqKNjaUfISuRpgCIWFKdRtTdq1VRDf3qIn7S/nuhfWE4w15 ;{id = 11387 (zsk), size = 1024b}";

        let DNSKEY {
            rdata: DNSKEYRData { public_key, .. },
            ..
        } = DNSKEY_INPUT2.parse()?;

        let expected = "AwEAAbEzD/uB2WK89f+PJ1Lyg5xvdt9mXge/R5tiQl8SEAUh/kfbn8jQiakH3HbBnBtdNXpjYrsmM7AxMmJLrp75dFMVnl5693/cY5k4dSk0BFJPQtBsZDn/7Q1rviQn0gqKNjaUfISuRpgCIWFKdRtTdq1VRDf3qIn7S/nuhfWE4w15";
        assert_eq!(expected, public_key);

        Ok(())
    }

    // dig DS com.
    const DS_INPUT: &str = "com.	7612	IN	DS	19718 13 2 8ACBB0CD28F41250A80A491389424D341522D946B0DA0C0291F2D3D7 71D7805A";

    #[test]
    fn ds() -> Result<()> {
        let ds @ DS {
            zone,
            ttl,
            key_tag,
            algorithm,
            digest_type,
            digest,
        } = &DS_INPUT.parse()?;

        assert_eq!(FQDN("com.")?, *zone);
        assert_eq!(7612, *ttl);
        assert_eq!(19718, *key_tag);
        assert_eq!(13, *algorithm);
        assert_eq!(2, *digest_type);
        let expected = "8ACBB0CD28F41250A80A491389424D341522D946B0DA0C0291F2D3D771D7805A";
        assert_eq!(expected, digest);

        let output = ds.to_string();
        assert_eq!(DS_INPUT, output);

        Ok(())
    }

    // dig NS .
    const NS_INPUT: &str = ".	86400	IN	NS	f.root-servers.net.";

    #[test]
    fn ns() -> Result<()> {
        let ns @ NS {
            zone,
            ttl,
            nameserver,
        } = &NS_INPUT.parse()?;

        assert_eq!(FQDN::ROOT, *zone);
        assert_eq!(86400, *ttl);
        assert_eq!("f.root-servers.net.", nameserver.as_str());

        let output = ns.to_string();
        assert_eq!(NS_INPUT, output);

        Ok(())
    }

    const NSEC_INPUT: &str =
        "hickory-dns.testing.	86400	IN	NSEC	primary1.hickory-dns.testing. NS SOA RRSIG NSEC DNSKEY";

    #[test]
    fn nsec() -> Result<()> {
        let nsec @ NSEC {
            fqdn,
            ttl,
            next_domain,
            record_types,
        } = &NSEC_INPUT.parse()?;

        assert_eq!("hickory-dns.testing.", fqdn.as_str());
        assert_eq!(86400, *ttl);
        assert_eq!("primary1.hickory-dns.testing.", next_domain.as_str());
        assert_eq!(
            [
                RecordType::NS,
                RecordType::SOA,
                RecordType::RRSIG,
                RecordType::NSEC,
                RecordType::DNSKEY,
            ],
            record_types.as_slice()
        );

        let output = nsec.to_string();
        assert_eq!(NSEC_INPUT, output);

        Ok(())
    }

    // dig +dnssec A unicorn.example.com.
    const NSEC3_INPUT: &str = "abhif1b25fhcda5amfk5hnrsh6jid2ki.example.com.	3571	IN	NSEC3	1 0 5 53BCBC5805D2B761  GVPMD82B8ER38VUEGP72I721LIH19RGR A NS SOA MX TXT AAAA RRSIG DNSKEY NSEC3PARAM";

    #[test]
    fn nsec3() -> Result<()> {
        let nsec3 @ NSEC3 {
            fqdn,
            ttl,
            hash_alg,
            flags,
            iterations,
            salt,
            next_hashed_owner_name,
            record_types,
        } = &NSEC3_INPUT.parse()?;

        assert_eq!(
            "abhif1b25fhcda5amfk5hnrsh6jid2ki.example.com.",
            fqdn.as_str()
        );
        assert_eq!(3571, *ttl);
        assert_eq!(1, *hash_alg);
        assert_eq!(0, *flags);
        assert_eq!(5, *iterations);
        assert_eq!("53BCBC5805D2B761", salt);
        assert_eq!("GVPMD82B8ER38VUEGP72I721LIH19RGR", next_hashed_owner_name);
        assert_eq!(
            [
                RecordType::A,
                RecordType::NS,
                RecordType::SOA,
                RecordType::MX,
                RecordType::TXT,
                RecordType::AAAA,
                RecordType::RRSIG,
                RecordType::DNSKEY,
                RecordType::NSEC3PARAM
            ],
            record_types.as_slice()
        );

        let output = nsec3.to_string();
        assert_eq!(NSEC3_INPUT, output);

        Ok(())
    }

    // dig NSEC3PARAM com.
    const NSEC3PARAM_INPUT: &str = "com.	86238	IN	NSEC3PARAM	1 0 0 -";

    #[test]
    fn nsec3param() -> Result<()> {
        let nsec3param @ NSEC3PARAM {
            zone,
            ttl,
            hash_alg,
            flags,
            iterations,
        } = &NSEC3PARAM_INPUT.parse()?;

        assert_eq!(FQDN("com.")?, *zone);
        assert_eq!(86238, *ttl);
        assert_eq!(1, *hash_alg);
        assert_eq!(0, *flags);
        assert_eq!(0, *iterations);

        let output = nsec3param.to_string();
        assert_eq!(NSEC3PARAM_INPUT, output);

        Ok(())
    }

    // dig +dnssec SOA .
    const RRSIG_INPUT: &str = ".	1800	IN	RRSIG	SOA 7 0 1800 20240306132701 20240207132701 11264 . wXpRU4elJPGYm2kgVVsIwGf1IkYJcQ3UE4mwmItWdxj0XWSWY07MO4Ll DMJgsE0u64Q/345Ck7+aQ904uLebwCvpFnsmkyCxk82XIAfHN9FiwzSy qoR/zZEvBONaej3vrvsqPwh8q/pvypLft9647HcFdwY0juzZsbrAaDAX 8WY=";

    #[test]
    fn rrsig() -> Result<()> {
        let rrsig @ RRSIG {
            fqdn,
            ttl,
            type_covered,
            algorithm,
            labels,
            original_ttl,
            signature_expiration,
            signature_inception,
            key_tag,
            signer_name,
            signature,
        } = &RRSIG_INPUT.parse()?;

        assert_eq!(FQDN::ROOT, *fqdn);
        assert_eq!(1800, *ttl);
        assert_eq!(RecordType::SOA, *type_covered);
        assert_eq!(7, *algorithm);
        assert_eq!(0, *labels);
        assert_eq!(1800, *original_ttl);
        assert_eq!(20240306132701, *signature_expiration);
        assert_eq!(20240207132701, *signature_inception);
        assert_eq!(11264, *key_tag);
        assert_eq!(FQDN::ROOT, *signer_name);
        let expected = "wXpRU4elJPGYm2kgVVsIwGf1IkYJcQ3UE4mwmItWdxj0XWSWY07MO4LlDMJgsE0u64Q/345Ck7+aQ904uLebwCvpFnsmkyCxk82XIAfHN9FiwzSyqoR/zZEvBONaej3vrvsqPwh8q/pvypLft9647HcFdwY0juzZsbrAaDAX8WY=";
        assert_eq!(expected, signature);

        let output = rrsig.to_string();
        assert_eq!(RRSIG_INPUT, output);

        Ok(())
    }

    // dig SOA .
    const SOA_INPUT: &str = ".	15633	IN	SOA	a.root-servers.net. nstld.verisign-grs.com. 2024020501 1800 900 604800 86400";

    #[test]
    fn soa() -> Result<()> {
        let soa: SOA = SOA_INPUT.parse()?;

        assert_eq!(".", soa.zone.as_str());
        assert_eq!(15633, soa.ttl);
        assert_eq!("a.root-servers.net.", soa.nameserver.as_str());
        assert_eq!("nstld.verisign-grs.com.", soa.admin.as_str());
        let settings = &soa.settings;
        assert_eq!(2024020501, settings.serial);
        assert_eq!(1800, settings.refresh);
        assert_eq!(900, settings.retry);
        assert_eq!(604800, settings.expire);
        assert_eq!(86400, settings.minimum);

        let output = soa.to_string();
        assert_eq!(SOA_INPUT, output);

        Ok(())
    }

    // from the `truncated_with_tcp_fallback.py` test server.
    const TXT_INPUT: &str = r#"example.testing.	0	IN	TXT	"protocol=TCP" "counter=0""#;

    #[test]
    fn txt() -> Result<()> {
        let txt: TXT = TXT_INPUT.parse()?;

        assert_eq!("example.testing.", txt.zone.as_str());
        assert_eq!(0, txt.ttl);
        assert_eq!(
            vec!["protocol=TCP".to_owned(), "counter=0".to_owned()],
            txt.character_strings
        );

        let output = txt.to_string();
        assert_eq!(TXT_INPUT, output);

        Ok(())
    }

    const CAA_INPUT: &str = "certs.example.com.	86400	IN	CAA	0 issue ca1.example.net";

    #[test]
    fn caa() -> Result<()> {
        let caa @ CAA {
            zone,
            ttl,
            flags,
            tag,
            value,
        } = &CAA_INPUT.parse()?;

        assert_eq!(FQDN("certs.example.com.").unwrap(), *zone);
        assert_eq!(86400, *ttl);
        assert_eq!(0, *flags);
        assert_eq!("issue", tag);
        assert_eq!("ca1.example.net", value);

        let output = caa.to_string();
        assert_eq!(CAA_INPUT, output);

        Ok(())
    }

    #[test]
    fn any() -> Result<()> {
        assert!(matches!(A_INPUT.parse()?, Record::A(..)));
        assert!(matches!(CAA_INPUT.parse()?, Record::CAA(..)));
        assert!(matches!(DNSKEY_INPUT.parse()?, Record::DNSKEY(..)));
        assert!(matches!(DS_INPUT.parse()?, Record::DS(..)));
        assert!(matches!(NS_INPUT.parse()?, Record::NS(..)));
        assert!(matches!(NSEC_INPUT.parse()?, Record::NSEC(..)));
        assert!(matches!(NSEC3_INPUT.parse()?, Record::NSEC3(..)));
        assert!(matches!(NSEC3PARAM_INPUT.parse()?, Record::NSEC3PARAM(..)));
        assert!(matches!(RRSIG_INPUT.parse()?, Record::RRSIG(..)));
        assert!(matches!(SOA_INPUT.parse()?, Record::SOA(..)));
        assert!(matches!(TXT_INPUT.parse()?, Record::TXT(..)));

        Ok(())
    }

    #[test]
    fn unknown_type_round_trip() -> Result<()> {
        assert_eq!(RecordType::from_str("type1000")?, RecordType::Unknown(1000));
        assert_eq!(RecordType::from_str("TYPE1000")?, RecordType::Unknown(1000));
        assert_eq!(RecordType::Unknown(1000).as_name(), "type1000");
        Ok(())
    }
}
