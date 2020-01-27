//! Contains the steps required to generate an authorization key.
use getrandom::getrandom;
use grammers_tl_types::{self as tl};
use num::bigint::{BigUint, ToBigUint};
use num::integer::Integer;
use num::traits::cast::ToPrimitive;
use std::error::Error;
use std::fmt;
use std::io;

/// Represents an error that occured during the generation of an
/// authorization key.
#[derive(Debug)]
pub enum AuthKeyGenError {
    /// The server's nonce did not match ours.
    BadNonce {
        client_nonce: [u8; 16],
        server_nonce: [u8; 16],
    },

    /// The server's PQ number was not of the right size.
    WrongSizePQ { size: usize, expected: usize },
}

impl Error for AuthKeyGenError {}

impl fmt::Display for AuthKeyGenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO better display
        write!(f, "{:?}", self)
    }
}

impl From<AuthKeyGenError> for io::Error {
    fn from(error: AuthKeyGenError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// Step 1. Generates a secure random nonce.
pub fn generate_nonce() -> [u8; 16] {
    let mut buffer = [0; 16];
    getrandom(&mut buffer).expect("failed to generate a secure nonce");
    buffer
}

/// Step 2. Validate the PQ response. Return `(p, q)` if it's valid.
pub fn validate_pq(
    client_nonce: &[u8; 16],
    res_pq: &tl::types::ResPQ,
) -> Result<u64, AuthKeyGenError> {
    if *client_nonce != res_pq.nonce {
        return Err(AuthKeyGenError::BadNonce {
            client_nonce: client_nonce.clone(),
            server_nonce: res_pq.nonce.clone(),
        });
    }

    if res_pq.pq.len() != 8 {
        return Err(AuthKeyGenError::WrongSizePQ {
            size: res_pq.pq.len(),
            expected: 8,
        });
    }

    let pq = {
        let mut buffer = [0; 8];
        buffer.copy_from_slice(&res_pq.pq);
        u64::from_be_bytes(buffer)
    };

    Ok(pq)
}

/// Step 3. Factorize PQ and construct the request for DH params.
pub fn construct_req_dh_params(pq: u64) {
    let (p, q) = factorize(pq);
}

/// Factorize the given number into its two prime factors.
///
/// The algorithm here is a faster variant of [Pollard's rho algorithm],
/// published by [Richard Brent], based on
/// https://comeoncodeon.wordpress.com/2010/09/18/pollard-rho-brent-integer-factorization/.
///
/// Pollard's rho algorithm: https://en.wikipedia.org/wiki/Pollard%27s_rho_algorithm
/// Richard Brent: https://maths-people.anu.edu.au/~brent/pd/rpb051i.pdf
fn factorize(pq: u64) -> (u64, u64) {
    // TODO try to clean-up this BigUint mess
    if pq % 2 == 0 {
        return (2, pq);
    }

    /// Convenience function to convert an unsigned 64 bit integer into a
    /// big unsigned integer.
    fn big(n: u64) -> BigUint {
        // Safe to unwrap because the numbers we have are valid.
        n.to_biguint().unwrap()
    }

    /// The opposite of `big`. This will panic if the caller did not make sure
    /// that the value fits within 64 bits.
    fn small(n: &BigUint) -> u64 {
        n.to_u64().unwrap()
    }

    /// Returns the smallet of two big numbers as unsigned integer.
    fn min(a: &BigUint, b: &BigUint) -> u64 {
        if a < b {
            small(a)
        } else {
            small(b)
        }
    }

    /// The positive difference of two big numbers.
    fn abs_sub(a: &BigUint, b: &BigUint) -> BigUint {
        if a > b {
            a - b
        } else {
            b - a
        }
    }

    // Random values in the range of 1..pq, chosen by fair dice roll.
    let mut y = big(1 * pq / 4);
    let mut c = big(2 * pq / 4);
    let mut m = big(3 * pq / 4);
    let mut g = big(1u64);
    let mut r = big(1u64);
    let mut q = big(1u64);
    let mut x = big(0u64);
    let mut ys = big(0u64);
    let pq = big(pq);

    while g == big(1) {
        x = y.clone();
        for i in 0..small(&r) {
            y = (y.modpow(&big(2), &pq) + &c) % &pq;
        }

        let mut k = big(0);
        while k < r && g == big(1) {
            ys = y.clone();
            for i in 0..min(&m, &(&r - &k)) {
                y = (y.modpow(&big(2), &pq) + &c) % &pq;
                q = (q * abs_sub(&x, &y)) % &pq;
            }

            g = q.gcd(&pq);
            k += &m;
        }

        r *= big(2);
    }

    if g == pq {
        while g == big(1) {
            ys = (ys.modpow(&big(2), &pq) + &c) % &pq;
            g = abs_sub(&x, &ys).gcd(&pq);
        }
    }

    let (p, q) = (small(&g), small(&(&pq / &g)));
    (p.min(q), p.max(q))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factorization() {
        let pq = factorize(1470626929934143021);
        assert_eq!(pq, (1206429347, 1218991343));
    }
}
