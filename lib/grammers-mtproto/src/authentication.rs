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
use grammers_crypto::hex;
use grammers_crypto::{factorize::factorize, rsa, AuthKey};
use grammers_tl_types::{self as tl, Cursor, Deserializable, RemoteCall, Serializable};
use num_bigint::{BigUint, ToBigUint};
use sha1::{Digest, Sha1};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

// NOTE! Turning this on will leak the key generation process to stdout!
// Should only be used for debugging purposes and generating test cases.
const TRACE_AUTH_GEN: bool = false;

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

    if TRACE_AUTH_GEN {
        println!("r {}", hex::to_hex(&random_bytes));
    }

    let res = do_step1(&random_bytes);
    if TRACE_AUTH_GEN {
        if let Ok((x, _)) = &res {
            println!("> {}", hex::to_hex(x));
        }
    }
    res
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
    if TRACE_AUTH_GEN {
        println!("< {}", hex::to_hex(response));
    }

    let random_bytes = {
        let mut buffer = [0; 32 + 224];
        getrandom(&mut buffer).expect("failed to generate secure data for auth key");
        buffer
    };

    if TRACE_AUTH_GEN {
        println!("r {}", hex::to_hex(&random_bytes));
    }

    let res = do_step2(data, response, &random_bytes);
    if TRACE_AUTH_GEN {
        if let Ok((x, _)) = &res {
            println!("> {}", hex::to_hex(x));
        }
    }
    res
}

fn do_step2(
    data: Step1,
    response: &[u8],
    random_bytes: &[u8; 32 + 224],
) -> Result<(Vec<u8>, Step2), Error> {
    // Step 2. Validate the PQ response. Return `(p, q)` if it's valid.
    let Step1 { nonce } = data;
    let tl::enums::ResPq::Pq(res_pq) =
        <tl::functions::ReqPqMulti as RemoteCall>::Return::from_bytes(response)?;

    check_nonce(&res_pq.nonce, &nonce)?;

    let check_pq = res_pq.pq.to_bytes();

    if check_pq.len() != 8 {
        return Err(Error::InvalidPQSize {
            size: check_pq.len(),
        });
    }

    let pq = {
        let mut buffer = [0; 8];
        buffer.copy_from_slice(&check_pq);
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
        let mut buffer = [0; 224];
        buffer.copy_from_slice(&random_bytes[32..]);
        buffer
    };

    // Convert (p, q) to bytes using the least amount of space possible.
    // If we don't do this, Telegram will respond with -404 as the message.
    let p_string = {
        let mut buffer = p.to_be_bytes().to_vec();
        if let Some(pos) = buffer.iter().position(|&b| b != 0) {
            buffer = buffer[pos..].to_vec();
        }
        String::from_bytes(&buffer).unwrap()
    };
    let q_string = {
        let mut buffer = q.to_be_bytes().to_vec();
        if let Some(pos) = buffer.iter().position(|&b| b != 0) {
            buffer = buffer[pos..].to_vec();
        }
        String::from_bytes(&buffer).unwrap()
    };

    // "pq is a representation of a natural number (in binary big endian format)"
    // https://core.telegram.org/mtproto/auth_key#dh-exchange-initiation
    let pq_inner_data = tl::enums::PQInnerData::Data(tl::types::PQInnerData {
        pq: res_pq.pq,
        p: p_string.clone(),
        q: q_string.clone(),
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
            p: p_string.clone(),
            q: q_string.clone(),
            public_key_fingerprint: fingerprint,
            encrypted_data: String::from_bytes(&ciphertext).unwrap(),
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
    if TRACE_AUTH_GEN {
        println!("< {}", hex::to_hex(response));
    }

    let random_bytes = {
        let mut buffer = [0; 256 + 16];
        getrandom(&mut buffer).expect("failed to generate secure data for auth key");
        buffer
    };

    if TRACE_AUTH_GEN {
        println!("r {}", hex::to_hex(&random_bytes));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_secs() as i32;

    let res = do_step3(data, response, &random_bytes, now);
    if TRACE_AUTH_GEN {
        if let Ok((x, _)) = &res {
            println!("> {}", hex::to_hex(x));
        }
    }
    res
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

            let sha = {
                let mut hasher = Sha1::new();
                hasher.update(new_nonce);
                hasher.finalize()
            };
            let new_nonce_hash = {
                let mut buffer = [0; 16];
                buffer.copy_from_slice(&sha[4..20]);
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
        grammers_crypto::decrypt_ige(&server_dh_params.encrypted_answer.to_bytes(), &key, &iv);

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
        let mut hasher = Sha1::new();
        hasher.update(&plain_text_answer[20..20 + plain_text_cursor.pos()]);
        hasher.finalize().into()
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
    let dh_prime = BigUint::from_bytes_be(&server_dh_inner.dh_prime.to_bytes());
    let g = server_dh_inner.g.to_biguint().unwrap();
    let g_a = BigUint::from_bytes_be(&server_dh_inner.g_a.to_bytes());

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
        g_b: g_b.to_string(),
    })
    .to_bytes();

    // sha1(client_dh_inner).digest() + client_dh_inner
    let sha = {
        let mut hasher = Sha1::new();
        hasher.update(&client_dh_inner);
        hasher.finalize()
    };

    let client_dh_inner_hashed = {
        let mut buffer = Vec::with_capacity(20 + client_dh_inner.len() + 16);

        buffer.extend(&sha);
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
            encrypted_data: String::from_bytes(&client_dh_encrypted).unwrap(),
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
    if TRACE_AUTH_GEN {
        println!("< {}", hex::to_hex(response));
    }

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

    if TRACE_AUTH_GEN {
        println!("a {}", hex::to_hex(&auth_key.to_bytes()));
        println!("o {}", time_offset);
        println!("s {}", first_salt);
    }

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
        // Production
        -3414540481677951611 => rsa::Key::new("29379598170669337022986177149456128565388431120058863768162556424047512191330847455146576344487764408661701890505066208632169112269581063774293102577308490531282748465986139880977280302242772832972539403531316010870401287642763009136156734339538042419388722777357134487746169093539093850251243897188928735903389451772730245253062963384108812842079887538976360465290946139638691491496062099570836476454855996319192747663615955633778034897140982517446405334423701359108810182097749467210509584293428076654573384828809574217079944388301239431309115013843331317877374435868468779972014486325557807783825502498215169806323", "65537").unwrap(),
        // Test
        -5595554452916591101 => rsa::Key::new("25342889448840415564971689590713473206898847759084779052582026594546022463853940585885215951168491965708222649399180603818074200620463776135424884632162512403163793083921641631564740959529419359595852941166848940585952337613333022396096584117954892216031229237302943701877588456738335398602461675225081791820393153757504952636234951323237820036543581047826906120927972487366805292115792231423684261262330394324750785450942589751755390156647751460719351439969059949569615302809050721500330239005077889855323917509948255722081644689442127297605422579707142646660768825302832201908302295573257427896031830742328565032949", "65537").unwrap(),

        _ => return None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emulate_successful_auth_key_gen_flow() -> Result<(), Error> {
        let step1_random = hex::from_hex("4e44b426241e8b839153122d44585ac6")
            .as_slice()
            .try_into()
            .unwrap();
        let step1_request = hex::from_hex("f18e7ebe4e44b426241e8b839153122d44585ac6");
        let step1_response = hex::from_hex("632416054e44b426241e8b839153122d44585ac665ba0b393e1094329eda2c42d62833030819546f942a11278d00000015c4b51c0300000003268d20df9858b2029f4ba16d109296216be86c022bb4c3");
        let step2_random = hex::from_hex("b9dce68b05ef760fa7edfefeff45aaa8afbac11dc3d333bc3132fd16ab816d63ed93c5bef9d0452add8164a2d5df5804277ee5a06fd4523372707ddbd8106d03766d76fb8bec672bdcddcd225f7766b83663b32a0fda1055175c5582edd10430937666be4fd15510ba5f19aa645973b6e4e9270efac25b58741635fe84dd0af07a4686f750bf34de1073f1e7fa24e9b01a76e537504bd52b8195e5b78c9af2baa982454e1a99eeae0f35944089ad12726d2433a2c18c9698a725364f9c4e939ce4f1aee3891e58b85de90c88cc2eaef5db1841a594c0edc13cb4b7480a7e564fe892f82282d03ed07eb5ceac6644247bb137241166fe194756dfcffd68c6c345").as_slice().try_into().unwrap();
        let step2_request = hex::from_hex("bee412d74e44b426241e8b839153122d44585ac665ba0b393e1094329eda2c42d62833030444b2e50d000000045e63ac8100000003268d20df9858b2fe0001007ec37ca8a84aa1b26d21bc8ac28b261ffa57b44e29f0d6722261e9b436059cc80ae9768a3ae4fbefe46cfbb76b88a1f80a1ebd95ae5d17bf655ed1015755e04c483a01cf4094a0830864054a71a0ac8a5ec34d6b24a69bf66c9654b32a8c65b0302718351b28f72a9a49610d5259b6edb6da37acc5fedc47d1a09c58df2c7eccbfaf54dfe123ebc253d9069f74e8be128051e5d280b3c9a5e8d3c6da344cb7374a6d410d4e088cc0eda3d8b1108ba4f4a85d79fbd2758000723780bc5459f59fd1cea1b511b77cc1411781d3feb57b14a97726cf3d2146cf43e648a69ff9cb5d48a31f543bd5bc3a023cf382d86d36bbfbbcb5e4a136acee25fd8e3e597e714d");
        let step2_response = hex::from_hex("5c07e8d04e44b426241e8b839153122d44585ac665ba0b393e1094329eda2c42d6283303fe500200fd064e91012ade621b26a48ac7dc8b2c8670ed67092a00fe8c936483e4b02822c3cc655aaffe00542e311df5abdaa645b1da85ca50a6c7b0e7cc7cb2b23d42c84e288bb3b5cfe313e1ebafe19833916df4d1f58dba62e0ac49cac17a31b8b0d57d43eefda546d67e80e311c4b213adec9635c73f75a18ffb26fb71391523bd5ddfcc8be51b36d6b2552394c511ec935d53811a981baca62a2b58cbfe96f1b35e118e5e17456994aea931839925c4578f281f3f129d28026ec80224617a9ca8c615a12fba9c53e774476567f07b01a59d2e6635e39c16dc0a54679f3b54b0482f1cbeac821147d93d7365f4e23fb5794eb5fd4ffdc6456638ea32f641f49ee705e7b0da71cb75753e2f4f80d5af07edb017948f332e34a9c5886b0c86281e0e7228d5a652a9faaf819f7686c099186169aaa377c136fac57b69b7f7b383aaece652f8dcb14e0dfb23e2a65330307a74c31c508cc504450fa208eee14d8bbead1c1f90ccfc183ae1d3345c62424ea3477776204e8fe69efbb6a27b168913d3babaca30aa1c9589d6655b2ad4cd59f67e9b3957ab3270d70afab9bd488a6c5f39ca739ca8947def00cdb8812152731710f5108235775a019d3b4986d6b720b05167b4ee731a10a29fc1e03c42e99d8ff5cf64f45070c2f5ce485ea5fddc281728b6e4d0dea561c9097e3f8a54b055b0c069a9f8207520f6429eb5225c985e3379f2cf6754f56d414fcd00d502e69223b911b915978e0890a9ef128715b828bf3fda3fee6c7b9b2621d971a6f7820f89f4c4c2ab29dec00007c3ec6cead64f7f5802d5e6a4a16a185cfbfced5351fa68380e");
        let step3_random = hex::from_hex("8fc3605a4604cbb5461fdeff439c761150083cdd502550558e92c730d46c9caf0b1b2d64d2c264942c50d98694fff604fdd2bd87f2cafb719bc55e65a1f60b08809660a650721c40d56fc9c792df1d463aad1718c6924b7bdffbe395f14633d33fc38ce47c18a1561b83a5c66d29f9e292637127471c3baab0028ae42796b689e53a7f9ab5f0ee6d3fb658d847c1abca509fc4ed0d45edbb1c946488910d8d78fa0767255b57a7c3898da8d26625bde40c5a0e80b581408ecd95a17d396dc7574a8ed3cbc4c085197ffaad29c18e577eb292aa8b98caa92efd6f9536049b5a7defc861e270eca90c55b9585405cb96f3e6ea754850b09e7a59ba5fd92d357982915d39752aaa2ec16b6cbde6a6c33971").as_slice().try_into().unwrap();
        let step3_request = hex::from_hex("1f5f04f54e44b426241e8b839153122d44585ac665ba0b393e1094329eda2c42d6283303fe500100def448d48c608480bab65df3f8990be8011f7b415a6f8113617bea749b8b0ea6a937987b18cc4dcce8197efdcf8d6ec6af7fc3364b4945df77e4a1ae9db7acea4abcd73247edb36bde20fc969c1d55717277afe0bc31a9ee99f7d822f91fa2dc69c868a19511b162d55e0814d0292b7708b67d57eb04569349d5a20ffe85c0141fc17e9bbbaf207bef56e66decda718c52c45273f868c2eff89bb06355cd515fbfe123d719b244234867d2889c9d0e4436ba644076e5014a78af60b2f0e1b30285f4f71539bcf8c506ccafd62cfcd1b040fe5e35bb30e519ad56d753100f604e3ea5d02409d74dd3ab0861227410f1e13591cf2a638347e6c6d0bcae14e0e8753313b51daee40a67407b5cc8b213856a290a0c7b6cda9ff9c58d69faaf6a748cff05512b69f1380f7a36843edecdc764048bc16d9808f353a9caf6d49ca8b717c8f6de037518a444931a7da2b80f16d0");
        let step3_response = hex::from_hex("34f7cb3b4e44b426241e8b839153122d44585ac665ba0b393e1094329eda2c42d628330313b781a0de4ab6bc7ab414cbe13f9f86");
        let expected_auth_key = hex::from_hex("7582e48ad36cd6eef7944ac9bd7027de9ee3202543b68850ac01e1221350f7174e6c3771c9d86b3075f777539c23d053e9da9a1510d49e8fa0ad76a016ce28bfe3543dde69959bc682dab762b95a36629a8438e65baa53cc79b551c23d555c7675a36f4ece90882ece497d28a903409b780a8a80516cb0f8534fee3a67530beb2b1929626e07c2a052c4870b18b0a626606ca05cb13668a65aee3fa32cbebf1b3a56532138cb22c017cac44a292021902eea9b9f906c6be19c9203c7bb3ebc5f1b2044d0a90cb008f7248c3ae4449e0895b6090abb04c24131c2948bd27d879ecb934e50a46671f987653385ab388e4fa1ddd4c95743111e08bf11fef1f8f739").as_slice().try_into().unwrap();
        let expected_time_offset = 0;
        let expected_first_salt = 4459407212920268508;

        let (request, data) = do_step1(&step1_random)?;
        assert_eq!(request, step1_request.to_vec());
        let response = step1_response;

        let (request, data) = do_step2(data, &response, &step2_random)?;
        assert_eq!(request, step2_request.to_vec());
        let response = step2_response;

        let step3_now = 1693436740;
        let (request, data) = do_step3(data, &response, &step3_random, step3_now)?;
        assert_eq!(request, step3_request.to_vec());
        let response = step3_response;

        let finished = create_key(data, &response)?;
        assert_eq!(
            finished,
            Finished {
                auth_key: expected_auth_key,
                time_offset: expected_time_offset,
                first_salt: expected_first_salt,
            }
        );

        Ok(())
    }
}
