// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Contains the steps required to generate an authorization key.
//!
//! # Examples
//!
//! ```no_run
//! use grammers_mtproto::authentication;
//!
//! fn send_data_to_server(request: &[u8]) -> Result<Vec<u8>, authentication::Error> {
//!     unimplemented!()
//! }
//!
//! fn main() -> Result<(), authentication::Error> {
//!     let (request, data) = authentication::step1()?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let (request, data) = authentication::step2(data, &response)?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let (request, data) = authentication::step3(data, &response)?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let authentication::Finished { auth_key, .. } = authentication::create_key(data, &response)?;
//!     // Now you have a secure `auth_key` to send encrypted messages to server.
//!     Ok(())
//! }
//! ```
use getrandom::getrandom;
use grammers_crypto::{factorize::factorize, rsa, AuthKey};
use grammers_tl_types::{self as tl, Cursor, Deserializable, RemoteCall, Serializable};
use num_bigint::{BigUint, ToBigUint};
use sha1::Sha1;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

/// Represents an error that occured during the generation of an
/// authorization key.
#[derive(Clone, Debug, PartialEq)]
pub enum Error {
    /// The response data was invalid and did not match our expectations.
    InvalidResponse {
        /// The inner error that caused the invalid response.
        error: tl::deserialize::Error,
    },

    /// The server's nonce did not match ours.
    InvalidNonce {
        /// The unexpected nonce that we got.
        got: [u8; 16],

        /// The expected nonce.
        expected: [u8; 16],
    },

    /// The server's PQ number was not of the right size.
    InvalidPQSize {
        /// The unexpected size that we got.
        size: usize,
    },

    /// None of the server fingerprints are known to us.
    UnknownFingerprints {
        /// The list of fingerprint that we got.
        fingerprints: Vec<i64>,
    },

    /// The server failed to send the Diffie-Hellman parameters.
    DHParamsFail,

    /// The server's nonce has changed during the key exchange.
    InvalidServerNonce {
        /// The unexpected nonce that we got.
        got: [u8; 16],

        /// The expected nonce.
        expected: [u8; 16],
    },

    /// The server's `encrypted_data` is not correctly padded.
    EncryptedResponseNotPadded {
        /// The non-padded length of the response.
        len: usize,
    },

    /// An error occured while trying to read the DH inner data.
    InvalidDhInnerData {
        /// The inner error that occured when reading the data.
        error: tl::deserialize::Error,
    },

    /// Some parameter (`g`, `g_a` or `g_b`) was out of range.
    GParameterOutOfRange {
        value: BigUint,
        low: BigUint,
        high: BigUint,
    },

    // The generation of Diffie-Hellman parameters is to be retried.
    DHGenRetry,

    // The generation of Diffie-Hellman parameters failed.
    DHGenFail,

    /// The plaintext answer hash did not match.
    InvalidAnswerHash {
        /// The unexpected hash that we got.
        got: [u8; 20],

        /// The expected hash.
        expected: [u8; 20],
    },

    // The new nonce hash did not match.
    InvalidNewNonceHash {
        /// The unexpected nonce that we got.
        got: [u8; 16],

        /// The expected nonce.
        expected: [u8; 16],
    },
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidResponse { error } => write!(f, "invalid server response: {}", error),
            Self::InvalidNonce { got, expected } => {
                write!(f, "invalid nonce: got {:?}, expected {:?}", got, expected)
            }
            Self::InvalidPQSize { size } => write!(f, "invalid pq size {}", size),
            Self::UnknownFingerprints { fingerprints } => {
                write!(f, "all server fingerprints are unknown: {:?}", fingerprints)
            }
            Self::DHParamsFail => write!(f, "the generation of DH parameters by the server failed"),
            Self::InvalidServerNonce { got, expected } => write!(
                f,
                "invalid server nonce: got {:?}, expected {:?}",
                got, expected
            ),
            Self::EncryptedResponseNotPadded { len } => write!(
                f,
                "the encrypted server response was {} bytes long, which is not correctly padded",
                len
            ),
            Self::InvalidDhInnerData { error } => {
                write!(f, "could not deserialize DH inner data: {}", error)
            }
            Self::GParameterOutOfRange { low, high, value } => write!(
                f,
                "the parameter g = {} was not in the range {}..{}",
                value, low, high
            ),
            Self::DHGenRetry => write!(f, "the generation of DH parameters should be retried"),
            Self::DHGenFail => write!(f, "the generation of DH parameters failed"),
            Self::InvalidAnswerHash { got, expected } => write!(
                f,
                "invalid answer hash: got {:?}, expected {:?}",
                got, expected
            ),
            Self::InvalidNewNonceHash { got, expected } => write!(
                f,
                "invalid new nonce hash: got {:?}, expected {:?}",
                got, expected
            ),
        }
    }
}

impl From<tl::deserialize::Error> for Error {
    fn from(error: tl::deserialize::Error) -> Self {
        Self::InvalidResponse { error }
    }
}

/// The data generated by [`step1`], needed for [`step2`].
///
/// [`step1`]: fn.step1.html
/// [`step2`]: fn.step2.html
pub struct Step1 {
    nonce: [u8; 16],
}

/// The data generated by [`step2`], needed for [`step3`].
///
/// [`step2`]: fn.step2.html
/// [`step3`]: fn.step3.html
pub struct Step2 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
}

/// The data generated by [`step3`], needed for [`create_key`].
///
/// [`step3`]: fn.step3.html
/// [`create_key`]: fn.create_key.html
pub struct Step3 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
    gab: BigUint,
    time_offset: i32,
}

/// The first step of the process to generate an authorization key.
pub fn step1() -> Result<(Vec<u8>, Step1), Error> {
    let random_bytes = {
        let mut buffer = [0; 16];
        getrandom(&mut buffer).expect("failed to generate secure data for auth key");
        buffer
    };

    do_step1(&random_bytes)
}

// n.b.: the `do_step` functions are pure so that they can be tested.
fn do_step1(random_bytes: &[u8; 16]) -> Result<(Vec<u8>, Step1), Error> {
    // Step 1. Generates a secure random nonce.
    let nonce = *random_bytes;
    Ok((
        tl::functions::ReqPqMulti { nonce }.to_bytes(),
        Step1 { nonce },
    ))
}

/// The second step of the process to generate an authorization key.
pub fn step2(data: Step1, response: &[u8]) -> Result<(Vec<u8>, Step2), Error> {
    let random_bytes = {
        let mut buffer = [0; 32 + 256];
        getrandom(&mut buffer).expect("failed to generate secure data for auth key");
        buffer
    };

    do_step2(data, response, &random_bytes)
}

fn do_step2(
    data: Step1,
    response: &[u8],
    random_bytes: &[u8; 32 + 256],
) -> Result<(Vec<u8>, Step2), Error> {
    // Step 2. Validate the PQ response. Return `(p, q)` if it's valid.
    let Step1 { nonce } = data;
    let tl::enums::ResPq::Pq(res_pq) =
        <tl::functions::ReqPqMulti as RemoteCall>::Return::from_bytes(response)?;

    check_nonce(&res_pq.nonce, &nonce)?;

    if res_pq.pq.len() != 8 {
        return Err(Error::InvalidPQSize {
            size: res_pq.pq.len(),
        });
    }

    let pq = {
        let mut buffer = [0; 8];
        buffer.copy_from_slice(&res_pq.pq);
        u64::from_be_bytes(buffer)
    };

    let (p, q) = factorize(pq);
    let new_nonce = {
        let mut buffer = [0; 32];
        buffer.copy_from_slice(&random_bytes[..32]);
        buffer
    };

    // Remove the now-used first part from our available random data.
    let random_bytes = {
        let mut buffer = [0; 256];
        buffer.copy_from_slice(&random_bytes[32..]);
        buffer
    };

    // Convert (p, q) to bytes using the least amount of space possible.
    // If we don't do this, Telegram will respond with -404 as the message.
    let p_bytes = {
        let mut buffer = p.to_be_bytes().to_vec();
        if let Some(pos) = buffer.iter().position(|&b| b != 0) {
            buffer = buffer[pos..].to_vec();
        }
        buffer
    };
    let q_bytes = {
        let mut buffer = q.to_be_bytes().to_vec();
        if let Some(pos) = buffer.iter().position(|&b| b != 0) {
            buffer = buffer[pos..].to_vec();
        }
        buffer
    };

    // "pq is a representation of a natural number (in binary big endian format)"
    // https://core.telegram.org/mtproto/auth_key#dh-exchange-initiation
    let pq_inner_data = tl::enums::PQInnerData::Data(tl::types::PQInnerData {
        pq: pq.to_be_bytes().to_vec(),
        p: p_bytes.clone(),
        q: q_bytes.clone(),
        nonce,
        server_nonce: res_pq.server_nonce,
        new_nonce,
    })
    .to_bytes();

    // sha_digest + data + random_bytes
    let fingerprint = match res_pq
        .server_public_key_fingerprints
        .iter()
        .cloned()
        .find(|&fingerprint| key_for_fingerprint(fingerprint).is_some())
    {
        Some(x) => x,
        None => {
            return Err(Error::UnknownFingerprints {
                fingerprints: res_pq.server_public_key_fingerprints.clone(),
            })
        }
    };

    // Safe to unwrap because we found it just above
    let key = key_for_fingerprint(fingerprint).unwrap();
    let ciphertext = rsa::encrypt_hashed(&pq_inner_data, &key, &random_bytes);

    Ok((
        tl::functions::ReqDhParams {
            nonce,
            server_nonce: res_pq.server_nonce,
            p: p_bytes,
            q: q_bytes,
            public_key_fingerprint: fingerprint,
            encrypted_data: ciphertext,
        }
        .to_bytes(),
        Step2 {
            nonce,
            server_nonce: res_pq.server_nonce,
            new_nonce,
        },
    ))
}

/// The third step of the process to generate an authorization key.
pub fn step3(data: Step2, response: &[u8]) -> Result<(Vec<u8>, Step3), Error> {
    let random_bytes = {
        let mut buffer = [0; 256 + 16];
        getrandom(&mut buffer).expect("failed to generate secure data for auth key");
        buffer
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_secs() as i32;

    do_step3(data, response, &random_bytes, now)
}

fn do_step3(
    data: Step2,
    response: &[u8],
    random_bytes: &[u8; 256 + 16],
    now: i32,
) -> Result<(Vec<u8>, Step3), Error> {
    let Step2 {
        nonce,
        server_nonce,
        new_nonce,
    } = data;
    let server_dh_params =
        <tl::functions::ReqDhParams as RemoteCall>::Return::from_bytes(response)?;

    // Step 3. Factorize PQ and construct the request for DH params.
    let server_dh_params = match server_dh_params {
        tl::enums::ServerDhParams::Fail(server_dh_params) => {
            // Even though this is a failing case, we should still perform
            // all the security checks.
            check_nonce(&server_dh_params.nonce, &nonce)?;
            check_server_nonce(&server_dh_params.server_nonce, &server_nonce)?;

            let new_nonce_hash = {
                let mut buffer = [0; 16];
                buffer.copy_from_slice(&Sha1::from(&new_nonce).digest().bytes()[4..20]);
                buffer
            };
            check_new_nonce_hash(&server_dh_params.new_nonce_hash, &new_nonce_hash)?;

            return Err(Error::DHParamsFail);
        }
        tl::enums::ServerDhParams::Ok(x) => x,
    };

    check_nonce(&server_dh_params.nonce, &nonce)?;
    check_server_nonce(&server_dh_params.server_nonce, &server_nonce)?;

    if server_dh_params.encrypted_answer.len() % 16 != 0 {
        return Err(Error::EncryptedResponseNotPadded {
            len: server_dh_params.encrypted_answer.len(),
        });
    }

    // Complete DH Exchange
    let (key, iv) = grammers_crypto::generate_key_data_from_nonce(&server_nonce, &new_nonce);

    // sha1 hash + plain text + padding
    let plain_text_answer =
        grammers_crypto::decrypt_ige(&server_dh_params.encrypted_answer, &key, &iv);

    let got_answer_hash = {
        let mut buffer = [0; 20];
        buffer.copy_from_slice(&plain_text_answer[..20]);
        buffer
    };

    // Use a cursor explicitly so we know where it ends (and most importantly
    // where the padding starts).
    let mut plain_text_cursor = Cursor::from_slice(&plain_text_answer[20..]);
    let server_dh_inner = match tl::enums::ServerDhInnerData::deserialize(&mut plain_text_cursor) {
        Ok(tl::enums::ServerDhInnerData::Data(x)) => x,
        Err(error) => return Err(Error::InvalidDhInnerData { error }),
    };

    let expected_answer_hash = {
        Sha1::from(&plain_text_answer[20..20 + plain_text_cursor.pos()])
            .digest()
            .bytes()
    };

    if got_answer_hash != expected_answer_hash {
        return Err(Error::InvalidAnswerHash {
            got: got_answer_hash,
            expected: expected_answer_hash,
        });
    }

    check_nonce(&server_dh_inner.nonce, &nonce)?;
    check_server_nonce(&server_dh_inner.server_nonce, &server_nonce)?;

    // Safe to unwrap because the numbers are valid
    let dh_prime = BigUint::from_bytes_be(&server_dh_inner.dh_prime);
    let g = server_dh_inner.g.to_biguint().unwrap();
    let g_a = BigUint::from_bytes_be(&server_dh_inner.g_a);

    let time_offset = server_dh_inner.server_time - now;

    let b = BigUint::from_bytes_be(&random_bytes[..256]);
    let g_b = g.modpow(&b, &dh_prime);
    let gab = g_a.modpow(&b, &dh_prime);

    // Remove the now-used first part from our available random data.
    let random_bytes = {
        let mut buffer = [0u8; 16];
        buffer.copy_from_slice(&random_bytes[256..]);
        buffer
    };

    // IMPORTANT: Apart from the conditions on the Diffie-Hellman prime
    // dh_prime and generator g, both sides are to check that g, g_a and
    // g_b are greater than 1 and less than dh_prime - 1. We recommend
    // checking that g_a and g_b are between 2^{2048-64} and
    // dh_prime - 2^{2048-64} as well.
    // (https://core.telegram.org/mtproto/auth_key#dh-key-exchange-complete)
    let one = BigUint::from_bytes_be(&[1]);
    check_g_in_range(&g, &one, &(&dh_prime - &one))?;
    check_g_in_range(&g_a, &one, &(&dh_prime - &one))?;
    check_g_in_range(&g_b, &one, &(&dh_prime - &one))?;

    let safety_range = one << (2048 - 64);
    check_g_in_range(&g_a, &safety_range, &(&dh_prime - &safety_range))?;
    check_g_in_range(&g_b, &safety_range, &(&dh_prime - &safety_range))?;

    // Prepare client DH Inner Data
    let client_dh_inner = tl::enums::ClientDhInnerData::Data(tl::types::ClientDhInnerData {
        nonce,
        server_nonce,
        retry_id: 0, // TODO use an actual retry_id
        g_b: g_b.to_bytes_be(),
    })
    .to_bytes();

    // sha1(client_dh_inner).digest() + client_dh_inner
    let client_dh_inner_hashed = {
        let mut buffer = Vec::with_capacity(20 + client_dh_inner.len() + 16);

        buffer.extend(&Sha1::from(&client_dh_inner).digest().bytes());
        buffer.extend(&client_dh_inner);

        // Make sure we pad it ourselves, or else `encrypt_ige` will,
        // introducing randomness.
        let pad_len = (16 - (buffer.len() % 16)) % 16;
        buffer.extend(&random_bytes[..pad_len]);

        buffer
    };

    let client_dh_encrypted = grammers_crypto::encrypt_ige(&client_dh_inner_hashed, &key, &iv);

    Ok((
        tl::functions::SetClientDhParams {
            nonce,
            server_nonce,
            encrypted_data: client_dh_encrypted,
        }
        .to_bytes(),
        Step3 {
            nonce,
            server_nonce,
            new_nonce,
            gab,
            time_offset,
        },
    ))
}

/// The final result of doing the authorization handshake, generated by [`create_key`].
///
/// [`create_key`]: fn.create_key.html
#[derive(Clone, Debug, PartialEq)]
pub struct Finished {
    pub auth_key: [u8; 256],
    pub time_offset: i32,
    pub first_salt: i64,
}

/// The last step of the process to generate an authorization key.
pub fn create_key(data: Step3, response: &[u8]) -> Result<Finished, Error> {
    let Step3 {
        nonce,
        server_nonce,
        new_nonce,
        gab,
        time_offset,
    } = data;
    let dh_gen = <tl::functions::SetClientDhParams as RemoteCall>::Return::from_bytes(response)?;

    struct DhGenData {
        nonce: [u8; 16],
        server_nonce: [u8; 16],
        new_nonce_hash: [u8; 16],
        nonce_number: u8,
    }

    let dh_gen = match dh_gen {
        tl::enums::SetClientDhParamsAnswer::DhGenOk(x) => DhGenData {
            nonce: x.nonce,
            server_nonce: x.server_nonce,
            new_nonce_hash: x.new_nonce_hash1,
            nonce_number: 1,
        },
        tl::enums::SetClientDhParamsAnswer::DhGenRetry(x) => DhGenData {
            nonce: x.nonce,
            server_nonce: x.server_nonce,
            new_nonce_hash: x.new_nonce_hash2,
            nonce_number: 2,
        },
        tl::enums::SetClientDhParamsAnswer::DhGenFail(x) => DhGenData {
            nonce: x.nonce,
            server_nonce: x.server_nonce,
            new_nonce_hash: x.new_nonce_hash3,
            nonce_number: 3,
        },
    };

    check_nonce(&dh_gen.nonce, &nonce)?;
    check_server_nonce(&dh_gen.server_nonce, &server_nonce)?;

    let auth_key = {
        let mut buffer = [0; 256];
        let gab_bytes = gab.to_bytes_be();
        let skip = buffer.len() - gab_bytes.len(); // gab might need less than 256 bytes
        buffer[skip..].copy_from_slice(&gab_bytes);
        AuthKey::from_bytes(buffer)
    };

    let new_nonce_hash = auth_key.calc_new_nonce_hash(&new_nonce, dh_gen.nonce_number);
    check_new_nonce_hash(&dh_gen.new_nonce_hash, &new_nonce_hash)?;

    let first_salt = {
        let mut buffer = [0; 8];
        buffer
            .iter_mut()
            .zip(&new_nonce[..8])
            .zip(&server_nonce[..8])
            .for_each(|((x, a), b)| *x = a ^ b);
        i64::from_le_bytes(buffer)
    };

    // 1 for DhGenOk
    if dh_gen.nonce_number == 1 {
        Ok(Finished {
            auth_key: auth_key.to_bytes(),
            time_offset,
            first_salt,
        })
    } else {
        Err(Error::DHGenFail)
    }
}

/// Helper function to avoid the boilerplate of checking for invalid nonce.
fn check_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidNonce {
            got: *got,
            expected: *expected,
        })
    }
}

/// Helper function to avoid the boilerplate of checking for invalid
/// server nonce.
fn check_server_nonce(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidServerNonce {
            got: *got,
            expected: *expected,
        })
    }
}

/// Helper function to avoid the boilerplate of checking for invalid
/// new nonce hash.
fn check_new_nonce_hash(got: &[u8; 16], expected: &[u8; 16]) -> Result<(), Error> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidNewNonceHash {
            got: *got,
            expected: *expected,
        })
    }
}

/// Helper function to avoid the boilerplate of checking for `g` not being
/// inside a valid range.
fn check_g_in_range(value: &BigUint, low: &BigUint, high: &BigUint) -> Result<(), Error> {
    if low < value && value < high {
        Ok(())
    } else {
        Err(Error::GParameterOutOfRange {
            value: value.clone(),
            low: low.clone(),
            high: high.clone(),
        })
    }
}

/// Find the RSA key's `(n, e)` pair for a certain fingerprint.
#[allow(clippy::unreadable_literal)]
fn key_for_fingerprint(fingerprint: i64) -> Option<rsa::Key> {
    Some(match fingerprint {
        // New
        847625836280919973 => rsa::Key::new("22081946531037833540524260580660774032207476521197121128740358761486364763467087828766873972338019078976854986531076484772771735399701424566177039926855356719497736439289455286277202113900509554266057302466528985253648318314129246825219640197356165626774276930672688973278712614800066037531599375044750753580126415613086372604312320014358994394131667022861767539879232149461579922316489532682165746762569651763794500923643656753278887871955676253526661694459370047843286685859688756429293184148202379356802488805862746046071921830921840273062124571073336369210703400985851431491295910187179045081526826572515473914151", "65537").unwrap(),
        1562291298945373506 => rsa::Key::new("23978758553106631992002580305620005835060400692492410830911253690968985161770919571023213268734637655796435779238577529598157303153929847488434262037216243092374262144086701552588446162198373312512977891135864544907383666560742498178155572733831904785232310227644261688873841336264291123806158164086416723396618993440700301670694812377102225720438542027067699276781356881649272759102712053106917756470596037969358935162126553921536961079884698448464480018715128825516337818216719699963463996161433765618041475321701550049005950467552064133935768219696743607832667385715968297285043180567281391541729832333512747963903", "65537").unwrap(),
        -5859577972006586033 => rsa::Key::new("22718646979021445086805300267873836551952264292680929983215333222894263271262525404635917732844879510479026727119219632282263022986926715926905675829369119276087034208478103497496557160062032769614235480480336458978483235018994623019124956728706285653879392359295937777480998285327855536342942377483433941973435757959758939732133845114873967169906896837881767555178893700532356888631557478214225236142802178882405660867509208028117895779092487773043163348085906022471454630364430126878252139917614178636934412103623869072904053827933244809215364242885476208852061471203189128281292392955960922615335169478055469443233", "65537").unwrap(),
        6491968696586960280 => rsa::Key::new("24037766801008650742980770419085067708599000106468359115503808361335510549334399420739246345211161442047800836519033544747025851693968269285475039555231773313724462564908666239840898204833183290939296455776367417572678362602041185421910456164281750840651140599266716366431221860463163678044675384797103831824697137394559208723253047225996994374103488753637228569081911062604259973219466527532055001206549020539767836549715548081391829906556645384762696840019083743214331245456023666332360278739093925808884746079174665122518196162846505196334513910135812480878181576802670132412681595747104670774040613733524133809153", "65537").unwrap(),

        // Old
        -4344800451088585951 => rsa::Key::new("24403446649145068056824081744112065346446136066297307473868293895086332508101251964919587745984311372853053253457835208829824428441874946556659953519213382748319518214765985662663680818277989736779506318868003755216402538945900388706898101286548187286716959100102939636333452457308619454821845196109544157601096359148241435922125602449263164512290854366930013825808102403072317738266383237191313714482187326643144603633877219028262697593882410403273959074350849923041765639673335775605842311578109726403165298875058941765362622936097839775380070572921007586266115476975819175319995527916042178582540628652481530373407", "65537").unwrap(),
        -7306692244673891685 => rsa::Key::new("25081407810410225030931722734886059247598515157516470397242545867550116598436968553551465554653745201634977779380884774534457386795922003815072071558370597290368737862981871277312823942822144802509055492512145589734772907225259038113414940384446493111736999668652848440655603157665903721517224934142301456312994547591626081517162758808439979745328030376796953660042629868902013177751703385501412640560275067171555763725421377065095231095517201241069856888933358280729674273422117201596511978645878544308102076746465468955910659145532699238576978901011112475698963666091510778777356966351191806495199073754705289253783", "65537").unwrap(),
        -5738946642031285640 => rsa::Key::new("22347337644621997830323797217583448833849627595286505527328214795712874535417149457567295215523199212899872122674023936713124024124676488204889357563104452250187725437815819680799441376434162907889288526863223004380906766451781702435861040049293189979755757428366240570457372226323943522935844086838355728767565415115131238950994049041950699006558441163206523696546297006014416576123345545601004508537089192869558480948139679182328810531942418921113328804749485349441503927570568778905918696883174575510385552845625481490900659718413892216221539684717773483326240872061786759868040623935592404144262688161923519030977", "65537").unwrap(),
        8205599988028290019 => rsa::Key::new("24573455207957565047870011785254215390918912369814947541785386299516827003508659346069416840622922416779652050319196701077275060353178142796963682024347858398319926119639265555410256455471016400261630917813337515247954638555325280392998950756512879748873422896798579889820248358636937659872379948616822902110696986481638776226860777480684653756042166610633513404129518040549077551227082262066602286208338952016035637334787564972991208252928951876463555456715923743181359826124083963758009484867346318483872552977652588089928761806897223231500970500186019991032176060579816348322451864584743414550721639495547636008351", "65537").unwrap(),

        _ => return None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulate_successful_auth_key_gen_flow() -> Result<(), Error> {
        let step1_random = [
            134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119, 241,
        ];
        let step1_request = [
            241, 142, 126, 190, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119,
            241,
        ];
        let step1_response = [
            99, 36, 22, 5, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119, 241,
            228, 177, 254, 82, 43, 118, 73, 81, 104, 145, 116, 35, 87, 201, 106, 26, 8, 32, 205,
            60, 176, 88, 123, 221, 113, 0, 0, 0, 21, 196, 181, 28, 2, 0, 0, 0, 2, 159, 75, 161,
            109, 16, 146, 150, 33, 107, 232, 108, 2, 43, 180, 195,
        ];
        let step2_random = [
            195, 148, 155, 120, 117, 242, 246, 19, 173, 110, 1, 18, 37, 133, 31, 132, 117, 162,
            226, 123, 6, 212, 126, 236, 148, 118, 40, 186, 21, 170, 122, 88, 123, 254, 46, 210,
            146, 11, 166, 205, 231, 146, 132, 135, 204, 245, 55, 238, 59, 159, 201, 99, 222, 189,
            69, 220, 43, 133, 113, 163, 190, 131, 95, 64, 4, 87, 41, 22, 197, 231, 172, 96, 196,
            121, 211, 108, 106, 186, 218, 231, 178, 214, 111, 115, 58, 217, 70, 44, 13, 52, 75,
            134, 150, 252, 149, 170, 103, 128, 41, 191, 26, 17, 173, 200, 58, 94, 252, 120, 41,
            161, 163, 114, 205, 218, 62, 41, 45, 242, 135, 6, 238, 16, 85, 23, 210, 215, 156, 13,
            159, 66, 158, 102, 208, 142, 51, 192, 16, 6, 126, 202, 155, 135, 131, 153, 56, 35, 180,
            25, 109, 70, 246, 210, 16, 26, 7, 246, 131, 3, 7, 82, 239, 131, 171, 222, 152, 232,
            189, 163, 184, 120, 132, 246, 123, 43, 102, 235, 3, 49, 165, 22, 65, 236, 26, 144, 88,
            101, 40, 60, 140, 182, 190, 202, 78, 109, 224, 139, 243, 244, 131, 133, 252, 61, 253,
            118, 56, 37, 159, 53, 48, 61, 181, 202, 217, 127, 179, 147, 143, 0, 217, 78, 170, 31,
            136, 64, 60, 71, 30, 19, 86, 40, 49, 59, 53, 155, 103, 64, 148, 91, 130, 171, 157, 166,
            31, 250, 122, 94, 236, 148, 106, 100, 134, 57, 71, 145, 160, 156, 10, 185, 84, 14, 1,
            86, 180, 28, 250, 191, 116, 241, 71, 127, 46, 146, 110, 54, 6, 146, 56, 126, 186, 127,
            233, 31, 24, 244, 97, 182, 51, 135,
        ];
        let step2_request = [
            190, 228, 18, 215, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119,
            241, 228, 177, 254, 82, 43, 118, 73, 81, 104, 145, 116, 35, 87, 201, 106, 26, 4, 90,
            137, 157, 155, 0, 0, 0, 4, 92, 191, 167, 227, 0, 0, 0, 33, 107, 232, 108, 2, 43, 180,
            195, 254, 0, 1, 0, 108, 15, 183, 223, 98, 253, 52, 254, 33, 51, 7, 255, 120, 46, 98,
            25, 200, 152, 153, 127, 129, 192, 53, 250, 55, 99, 90, 91, 88, 88, 145, 138, 201, 148,
            173, 47, 119, 70, 24, 31, 124, 57, 121, 66, 76, 29, 220, 225, 124, 23, 78, 157, 52, 1,
            46, 73, 39, 97, 227, 66, 124, 206, 155, 142, 208, 189, 188, 70, 42, 235, 83, 198, 157,
            144, 47, 6, 162, 29, 172, 91, 153, 193, 227, 173, 175, 197, 164, 200, 26, 63, 3, 196,
            102, 117, 230, 30, 208, 188, 36, 62, 54, 131, 204, 90, 37, 11, 166, 255, 226, 89, 108,
            159, 187, 107, 66, 13, 62, 240, 146, 0, 21, 29, 131, 180, 88, 152, 43, 231, 128, 234,
            92, 70, 45, 54, 155, 64, 253, 105, 207, 135, 185, 242, 254, 87, 236, 3, 99, 107, 63,
            248, 5, 144, 13, 180, 217, 60, 31, 64, 82, 231, 103, 217, 186, 176, 139, 155, 147, 11,
            168, 55, 85, 86, 8, 229, 67, 62, 227, 51, 168, 33, 192, 214, 228, 161, 17, 184, 38,
            120, 193, 31, 87, 116, 224, 159, 210, 122, 64, 9, 243, 119, 158, 4, 26, 111, 255, 44,
            133, 76, 217, 142, 231, 29, 243, 61, 145, 104, 243, 85, 126, 206, 23, 24, 33, 229, 45,
            181, 113, 120, 85, 134, 179, 210, 107, 8, 13, 162, 248, 69, 51, 69, 218, 199, 12, 104,
            51, 70, 64, 254, 100, 34, 14, 138, 121, 228, 240, 34,
        ];
        let step2_response = [
            92, 7, 232, 208, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119,
            241, 228, 177, 254, 82, 43, 118, 73, 81, 104, 145, 116, 35, 87, 201, 106, 26, 254, 80,
            2, 0, 58, 77, 67, 36, 153, 27, 168, 93, 247, 39, 91, 145, 230, 1, 236, 4, 36, 167, 96,
            38, 89, 49, 229, 19, 80, 71, 31, 51, 114, 5, 160, 2, 248, 43, 199, 59, 217, 29, 132,
            30, 119, 111, 40, 31, 22, 180, 147, 73, 171, 112, 90, 235, 86, 66, 206, 175, 169, 28,
            63, 143, 24, 173, 113, 6, 168, 61, 91, 226, 157, 24, 55, 216, 115, 12, 241, 163, 253,
            222, 83, 102, 119, 136, 36, 88, 191, 20, 23, 20, 38, 4, 192, 228, 133, 0, 148, 24, 54,
            119, 50, 198, 208, 199, 99, 226, 83, 57, 60, 108, 185, 67, 55, 125, 193, 216, 181, 142,
            69, 121, 34, 44, 144, 191, 87, 100, 51, 53, 222, 86, 225, 164, 59, 137, 147, 212, 92,
            220, 23, 101, 223, 115, 97, 124, 217, 187, 91, 42, 255, 221, 59, 167, 11, 37, 226, 70,
            14, 223, 239, 175, 128, 161, 64, 5, 12, 196, 35, 185, 16, 73, 15, 41, 166, 1, 102, 217,
            216, 232, 170, 242, 93, 250, 155, 133, 179, 41, 89, 26, 10, 169, 211, 225, 223, 196,
            115, 225, 211, 126, 116, 227, 254, 249, 87, 187, 11, 53, 210, 195, 163, 84, 13, 185,
            146, 166, 73, 198, 9, 145, 197, 93, 97, 137, 216, 80, 33, 242, 162, 15, 22, 9, 74, 158,
            11, 44, 136, 208, 165, 62, 196, 5, 199, 87, 115, 244, 47, 216, 16, 254, 109, 75, 1,
            100, 66, 213, 113, 147, 84, 214, 4, 202, 87, 0, 25, 7, 146, 202, 56, 104, 19, 74, 242,
            174, 249, 29, 186, 200, 204, 143, 220, 130, 230, 34, 94, 131, 208, 164, 210, 154, 153,
            146, 60, 21, 30, 35, 74, 32, 136, 130, 89, 17, 214, 20, 62, 106, 204, 16, 101, 159, 49,
            243, 123, 126, 168, 251, 221, 130, 209, 129, 96, 44, 48, 76, 65, 78, 135, 140, 204, 9,
            90, 50, 7, 24, 129, 159, 92, 104, 140, 36, 201, 30, 94, 10, 13, 74, 148, 44, 182, 76,
            6, 76, 157, 214, 237, 52, 125, 87, 181, 205, 188, 117, 210, 77, 73, 152, 3, 172, 168,
            235, 90, 237, 195, 35, 28, 142, 194, 22, 137, 227, 252, 61, 209, 120, 156, 82, 246, 30,
            231, 179, 191, 83, 192, 239, 244, 33, 222, 72, 58, 55, 83, 83, 105, 23, 90, 195, 45,
            50, 66, 38, 169, 212, 44, 52, 37, 126, 64, 18, 39, 106, 125, 112, 250, 121, 139, 85,
            171, 161, 59, 183, 252, 249, 159, 77, 246, 58, 192, 254, 164, 84, 226, 242, 241, 69,
            125, 14, 45, 198, 54, 0, 145, 212, 247, 214, 11, 54, 76, 200, 215, 7, 239, 81, 24, 206,
            61, 241, 136, 223, 214, 52, 13, 251, 206, 180, 176, 243, 119, 126, 218, 128, 192, 197,
            202, 163, 2, 178, 65, 210, 49, 188, 239, 72, 242, 49, 55, 0, 193, 90, 133, 112, 161,
            135, 86, 47, 136, 224, 16, 121, 7, 98, 79, 34, 225, 224, 98, 64, 128, 248, 159, 225,
            99, 99, 53, 194, 168, 208, 176, 38, 151, 219, 20, 60, 48, 46, 250, 36, 66, 154, 202,
            12, 239, 51, 1, 64, 43, 238, 81, 154, 225, 29, 139, 54, 2, 37, 232, 28, 165, 185, 78,
            57, 193, 206, 147, 204, 228, 222, 200, 37, 156, 20, 206, 246, 170, 2, 26, 161, 195,
            227, 181, 118, 210, 110, 37, 61, 99, 14, 168, 189, 250, 40, 239, 69, 232, 105, 207,
            235,
        ];
        let step3_random = [
            71, 146, 122, 10, 27, 21, 98, 145, 20, 44, 248, 247, 59, 3, 41, 161, 204, 108, 222,
            159, 182, 237, 137, 19, 53, 175, 250, 195, 62, 8, 58, 127, 238, 218, 95, 111, 233, 213,
            225, 87, 225, 157, 201, 19, 98, 60, 54, 204, 30, 211, 135, 97, 99, 8, 232, 249, 51,
            242, 7, 152, 152, 36, 124, 233, 93, 240, 43, 196, 97, 108, 116, 132, 41, 23, 24, 188,
            221, 3, 148, 27, 102, 82, 175, 226, 148, 95, 66, 29, 215, 67, 213, 31, 114, 233, 174,
            234, 204, 12, 117, 217, 31, 75, 50, 32, 204, 201, 218, 245, 236, 61, 113, 143, 94, 42,
            245, 95, 24, 34, 189, 6, 28, 238, 193, 192, 94, 72, 176, 9, 254, 20, 186, 22, 180, 132,
            79, 164, 33, 43, 202, 241, 85, 93, 31, 86, 91, 250, 134, 45, 119, 250, 109, 76, 244,
            85, 242, 93, 172, 83, 125, 5, 187, 70, 131, 67, 45, 122, 236, 168, 77, 113, 63, 218,
            34, 190, 227, 11, 214, 100, 190, 139, 57, 30, 150, 240, 156, 44, 113, 146, 182, 29,
            209, 178, 235, 48, 192, 6, 25, 83, 194, 140, 69, 14, 111, 98, 30, 100, 147, 58, 19,
            249, 65, 105, 232, 169, 78, 10, 234, 218, 125, 180, 12, 177, 1, 207, 119, 162, 96, 174,
            105, 106, 9, 165, 110, 64, 157, 105, 113, 132, 109, 64, 230, 39, 236, 130, 24, 151,
            137, 44, 92, 239, 60, 20, 227, 18, 43, 131, 182, 32, 130, 22, 0, 81, 255, 206, 147,
            219, 234, 93, 175, 135, 214, 30,
        ];
        let step3_request = [
            31, 95, 4, 245, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119, 241,
            228, 177, 254, 82, 43, 118, 73, 81, 104, 145, 116, 35, 87, 201, 106, 26, 254, 80, 1, 0,
            37, 116, 53, 109, 137, 150, 112, 137, 180, 146, 216, 87, 151, 235, 7, 74, 192, 156,
            169, 188, 70, 225, 9, 98, 86, 152, 15, 85, 162, 27, 242, 231, 228, 187, 36, 86, 151,
            88, 55, 36, 195, 11, 120, 162, 45, 78, 40, 29, 38, 247, 206, 253, 210, 15, 19, 180, 37,
            212, 202, 249, 104, 62, 1, 15, 244, 121, 194, 238, 135, 134, 140, 175, 193, 244, 117,
            92, 237, 101, 101, 154, 103, 143, 15, 39, 104, 13, 96, 233, 31, 154, 15, 128, 63, 37,
            100, 165, 167, 65, 157, 212, 123, 241, 33, 130, 198, 212, 140, 249, 250, 84, 159, 137,
            15, 138, 99, 175, 252, 112, 75, 7, 113, 5, 47, 72, 24, 211, 229, 210, 57, 232, 187,
            248, 96, 142, 92, 238, 155, 239, 178, 188, 77, 217, 15, 23, 192, 127, 96, 140, 174,
            223, 90, 78, 92, 254, 148, 255, 77, 118, 82, 78, 48, 207, 10, 2, 138, 23, 227, 113,
            251, 245, 247, 118, 226, 242, 245, 244, 7, 96, 35, 195, 13, 53, 211, 179, 127, 195,
            185, 3, 122, 64, 182, 116, 222, 103, 188, 206, 182, 74, 2, 94, 161, 149, 161, 248, 173,
            92, 225, 89, 233, 110, 125, 153, 172, 156, 117, 214, 251, 235, 77, 110, 22, 230, 184,
            233, 218, 51, 12, 37, 43, 11, 100, 164, 78, 25, 112, 131, 76, 211, 99, 113, 22, 206,
            83, 51, 164, 5, 155, 60, 83, 248, 100, 71, 104, 189, 141, 114, 248, 60, 6, 74, 151,
            141, 93, 136, 179, 107, 182, 78, 134, 62, 119, 5, 19, 101, 155, 184, 141, 41, 68, 233,
            73, 255, 100, 135, 117, 230, 238, 231, 141, 162, 225, 118, 101, 204, 182, 36, 155, 119,
            70, 120, 211, 117, 173, 217, 240, 4, 200, 175, 151, 65, 3, 255, 52, 1, 210, 18, 130,
            194, 191, 27, 247, 194, 143, 152, 195, 239, 172, 241, 121, 86, 99, 20, 89, 239, 228,
            193,
        ];
        let step3_response = [
            52, 247, 203, 59, 134, 212, 37, 230, 70, 13, 226, 160, 72, 38, 51, 17, 95, 143, 119,
            241, 228, 177, 254, 82, 43, 118, 73, 81, 104, 145, 116, 35, 87, 201, 106, 26, 22, 71,
            136, 56, 52, 202, 123, 253, 226, 115, 162, 78, 56, 200, 200, 179,
        ];
        let expected_auth_key = [
            11, 26, 74, 209, 176, 167, 145, 139, 118, 63, 183, 39, 60, 35, 202, 64, 8, 220, 16,
            122, 140, 136, 138, 125, 231, 86, 235, 147, 133, 3, 136, 229, 192, 82, 160, 237, 54,
            129, 102, 118, 132, 204, 151, 124, 58, 248, 245, 193, 190, 43, 162, 121, 84, 160, 147,
            10, 58, 227, 70, 51, 51, 48, 83, 130, 184, 5, 192, 135, 138, 167, 41, 203, 43, 228,
            182, 139, 114, 9, 93, 150, 220, 45, 53, 250, 96, 82, 171, 152, 165, 231, 3, 4, 216,
            141, 106, 150, 6, 230, 36, 197, 230, 222, 132, 148, 173, 139, 14, 87, 200, 183, 198,
            98, 144, 208, 26, 159, 253, 109, 17, 111, 183, 88, 94, 111, 242, 5, 88, 253, 154, 64,
            27, 47, 146, 82, 241, 158, 245, 232, 74, 163, 132, 141, 26, 157, 16, 20, 80, 19, 166,
            118, 140, 248, 81, 223, 218, 233, 0, 199, 245, 49, 142, 38, 35, 168, 169, 171, 205,
            111, 59, 229, 10, 167, 139, 159, 217, 64, 164, 157, 46, 250, 196, 61, 221, 132, 156,
            208, 38, 246, 24, 86, 208, 18, 30, 19, 21, 215, 193, 145, 210, 179, 10, 99, 219, 237,
            22, 11, 95, 71, 78, 106, 140, 112, 79, 244, 76, 185, 16, 216, 151, 2, 213, 203, 209,
            232, 130, 22, 237, 67, 21, 10, 239, 11, 170, 137, 36, 183, 28, 54, 125, 172, 97, 100,
            215, 159, 48, 24, 243, 221, 6, 142, 52, 189, 179, 18, 63, 224,
        ];

        let (request, data) = do_step1(&step1_random)?;
        assert_eq!(request, step1_request.to_vec());
        let response = step1_response;

        let (request, data) = do_step2(data, &response, &step2_random)?;
        assert_eq!(request, step2_request.to_vec());
        let response = step2_response;

        let step3_now = 1580236449;
        let (request, data) = do_step3(data, &response, &step3_random, step3_now)?;
        assert_eq!(request, step3_request.to_vec());
        let response = step3_response;

        let finished = create_key(data, &response)?;
        assert_eq!(
            finished,
            Finished {
                auth_key: expected_auth_key,
                time_offset: 0,
                first_salt: 4809708467028043047,
            }
        );

        Ok(())
    }
}
