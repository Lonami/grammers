//! Contains the steps required to generate an authorization key.
use getrandom::getrandom;
use grammers_crypto::{self, AuthKey};
use grammers_tl_types::{self as tl, Deserializable, Serializable, RPC};
use num::bigint::{BigUint, ToBigUint};
use num::integer::Integer;
use num::traits::cast::ToPrimitive;
use sha1::Sha1;
use std::error::Error;
use std::fmt;
use std::io;

/// Represents an error that occured during the generation of an
/// authorization key.
#[derive(Debug)]
pub enum AuthKeyGenError {
    /// The response data was invalid and did not match our expectations.
    InvalidResponse,

    /// The server's nonce did not match ours.
    BadNonce {
        got: [u8; 16],
        expected: [u8; 16],
    },

    /// The server's PQ number was not of the right size.
    WrongSizePQ {
        size: usize,
        expected: usize,
    },

    /// None of the server fingerprints are known to us.
    UnknownFingerprint,

    /// The server failed to send the Diffie-Hellman parameters.
    ServerDHParamsFail,

    /// The server's nonce has changed during the key exchange.
    BadServerNonce {
        got: [u8; 16],
        expected: [u8; 16],
    },

    /// The server's `encrypted_data` is not correctly padded.
    EncryptedResponseNotPadded,

    /// An error occured while trying to read the DH inner data.
    InvalidDHInnerData {
        error: io::Error,
    },

    /// Some parameter (`g`, `g_a` or `g_b`) was out of range.
    GParameterOutOfRange {
        low: BigUint,
        high: BigUint,
        value: BigUint,
    },

    // The generation of Diffie-Hellman parameters is to be retried.
    DHGenRetry,

    // The generation of Diffie-Hellman parameters failed.
    DHGenFail,

    // The new nonce hash did not match.
    BadNonceHash,
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

impl From<io::Error> for AuthKeyGenError {
    fn from(_error: io::Error) -> Self {
        Self::InvalidResponse
    }
}

pub struct Step1 {
    nonce: [u8; 16],
}

pub struct Step2 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
}

pub struct Step3 {
    nonce: [u8; 16],
    server_nonce: [u8; 16],
    new_nonce: [u8; 32],
    gab: BigUint,
    time_offset: i32,
}

pub fn step1() -> Result<(Vec<u8>, Step1), AuthKeyGenError> {
    // Step 1. Generates a secure random nonce.
    let nonce = {
        let mut buffer = [0; 16];
        getrandom(&mut buffer).expect("failed to generate a secure nonce");
        buffer
    };

    Ok((
        tl::functions::ReqPqMulti {
            nonce: nonce.clone(),
        }
        .to_bytes(),
        Step1 { nonce },
    ))
}

pub fn step2(data: Step1, response: Vec<u8>) -> Result<(Vec<u8>, Step2), AuthKeyGenError> {
    // Step 2. Validate the PQ response. Return `(p, q)` if it's valid.
    let Step1 { nonce } = data;
    let res_pq = match <tl::functions::ReqPqMulti as RPC>::Return::from_bytes(&response)? {
        tl::enums::ResPQ::ResPQ(x) => x,
    };

    if nonce != res_pq.nonce {
        return Err(AuthKeyGenError::BadNonce {
            got: res_pq.nonce.clone(),
            expected: nonce.clone(),
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

    let (p, q) = factorize(pq);
    let new_nonce = {
        let mut buffer = [0; 32];
        getrandom(&mut buffer).expect("failed to generate a secure nonce");
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
    let pq_inner_data = tl::enums::PQInnerData::PQInnerData(tl::types::PQInnerData {
        pq: pq.to_be_bytes().to_vec(),
        p: p_bytes.clone(),
        q: q_bytes.clone(),
        nonce: nonce.clone(),
        server_nonce: res_pq.server_nonce.clone(),
        new_nonce: new_nonce.clone(),
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
        None => return Err(AuthKeyGenError::UnknownFingerprint),
    };

    // Safe to unwrap because we found it just above
    let (n, e) = key_for_fingerprint(fingerprint).unwrap();
    let ciphertext = rsa_encrypt(&pq_inner_data, n, e, true);

    Ok((
        tl::functions::ReqDHParams {
            nonce: nonce.clone(),
            server_nonce: res_pq.server_nonce.clone(),
            p: p_bytes.clone(),
            q: q_bytes.clone(),
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

pub fn step3(data: Step2, response: Vec<u8>) -> Result<(Vec<u8>, Step3), AuthKeyGenError> {
    let Step2 {
        nonce,
        server_nonce,
        new_nonce,
    } = data;
    let server_dh_params = <tl::functions::ReqDHParams as RPC>::Return::from_bytes(&response)?;

    // Step 3. Factorize PQ and construct the request for DH params.
    let server_dh_params = match server_dh_params {
        tl::enums::ServerDHParams::ServerDHParamsFail(_) => {
            // TODO also validate nonce, server_nonce and new_nonce_hash here
            // new_nonce_hash = sha1(new_nonce).digest()[4..20]
            return Err(AuthKeyGenError::ServerDHParamsFail);
        }
        tl::enums::ServerDHParams::ServerDHParamsOk(x) => x,
    };

    if server_dh_params.nonce != nonce {
        return Err(AuthKeyGenError::BadNonce {
            got: server_dh_params.nonce.clone(),
            expected: nonce.clone(),
        });
    }
    if server_dh_params.server_nonce != server_nonce {
        return Err(AuthKeyGenError::BadServerNonce {
            got: server_dh_params.server_nonce.clone(),
            expected: server_nonce.clone(),
        });
    }
    if server_dh_params.encrypted_answer.len() % 16 != 0 {
        return Err(AuthKeyGenError::EncryptedResponseNotPadded);
    }

    // Complete DH Exchange
    let (key, iv) = grammers_crypto::generate_key_data_from_nonce(&server_nonce, &new_nonce);

    let plain_text_answer =
        grammers_crypto::decrypt_ige(&server_dh_params.encrypted_answer, &key, &iv);

    // TODO validate this hashsum
    let _hashsum = &plain_text_answer[..20];
    let server_dh_inner = match tl::enums::ServerDHInnerData::from_bytes(&plain_text_answer[20..]) {
        Ok(tl::enums::ServerDHInnerData::ServerDHInnerData(x)) => x,
        Err(error) => return Err(AuthKeyGenError::InvalidDHInnerData { error }),
    };

    if server_dh_inner.nonce != nonce {
        return Err(AuthKeyGenError::BadNonce {
            got: server_dh_inner.nonce.clone(),
            expected: nonce.clone(),
        });
    }
    if server_dh_inner.server_nonce != server_nonce {
        return Err(AuthKeyGenError::BadServerNonce {
            got: server_dh_inner.server_nonce.clone(),
            expected: server_nonce.clone(),
        });
    }

    // Safe to unwrap because the numbers are valid
    let dh_prime = BigUint::from_bytes_be(&server_dh_inner.dh_prime);
    let g = server_dh_inner.g.to_biguint().unwrap();
    let g_a = BigUint::from_bytes_be(&server_dh_inner.g_a);
    //let time_offset = server_dh_inner.server_time - int(time.time())
    let time_offset = 0; // TODO

    let b = {
        let mut buffer = [0; 256];
        getrandom(&mut buffer).expect("failed to generate a secure b value");
        BigUint::from_bytes_be(&buffer)
    };
    let g_b = g.modpow(&b, &dh_prime);
    let gab = g_a.modpow(&b, &dh_prime);

    // IMPORTANT: Apart from the conditions on the Diffie-Hellman prime
    // dh_prime and generator g, both sides are to check that g, g_a and
    // g_b are greater than 1 and less than dh_prime - 1. We recommend
    // checking that g_a and g_b are between 2^{2048-64} and
    // dh_prime - 2^{2048-64} as well.
    // (https://core.telegram.org/mtproto/auth_key#dh-key-exchange-complete)
    //if not (1 < g < (dh_prime - 1)):
    //    raise SecurityError('g_a is not within (1, dh_prime - 1)')

    //if not (1 < g_a < (dh_prime - 1)):
    //    raise SecurityError('g_a is not within (1, dh_prime - 1)')

    //if not (1 < g_b < (dh_prime - 1)):
    //    raise SecurityError('g_b is not within (1, dh_prime - 1)')

    //let safety_range = 2 ** (2048 - 64)
    //if not (safety_range <= g_a <= (dh_prime - safety_range)):
    //    raise SecurityError('g_a is not within (2^{2048-64}, dh_prime - 2^{2048-64})')

    //if not (safety_range <= g_b <= (dh_prime - safety_range)):
    //    raise SecurityError('g_b is not within (2^{2048-64}, dh_prime - 2^{2048-64})')

    // Prepare client DH Inner Data
    let client_dh_inner =
        tl::enums::ClientDHInnerData::ClientDHInnerData(tl::types::ClientDHInnerData {
            nonce: nonce.clone(),
            server_nonce: server_nonce.clone(),
            retry_id: 0, // TODO use an actual retry_id
            g_b: g_b.to_bytes_be(),
        })
        .to_bytes();

    // sha1(client_dh_inner).digest() + client_dh_inner
    let client_dh_inner_hashed = {
        let mut buffer = Vec::with_capacity(20 + client_dh_inner.len());

        let mut hasher = Sha1::new();
        hasher.update(&client_dh_inner);

        buffer.extend(&hasher.digest().bytes());
        buffer.extend(&client_dh_inner);
        buffer
    };

    let client_dh_encrypted = grammers_crypto::encrypt_ige(&client_dh_inner_hashed, &key, &iv);

    Ok((
        tl::functions::SetClientDHParams {
            nonce: nonce.clone(),
            server_nonce: server_nonce.clone(),
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

pub fn create_key(data: Step3, response: Vec<u8>) -> Result<(AuthKey, i32), AuthKeyGenError> {
    let Step3 {
        nonce,
        server_nonce,
        new_nonce,
        gab,
        time_offset,
    } = data;
    let dh_gen = <tl::functions::SetClientDHParams as RPC>::Return::from_bytes(&response)?;

    // TODO validate nonce and server_nonce for failing cases
    let (nonce_number, dh_gen) = match dh_gen {
        tl::enums::SetClientDHParamsAnswer::DhGenOk(x) => (1, x),
        tl::enums::SetClientDHParamsAnswer::DhGenRetry(_) => {
            return Err(AuthKeyGenError::DHGenRetry);
        }
        tl::enums::SetClientDHParamsAnswer::DhGenFail(_) => {
            return Err(AuthKeyGenError::DHGenFail);
        }
    };

    // TODO create a fn to reuse this check
    if dh_gen.nonce != nonce {
        return Err(AuthKeyGenError::BadNonce {
            got: dh_gen.nonce.clone(),
            expected: nonce.clone(),
        });
    }
    if dh_gen.server_nonce != server_nonce {
        return Err(AuthKeyGenError::BadServerNonce {
            got: dh_gen.server_nonce.clone(),
            expected: server_nonce.clone(),
        });
    }

    let auth_key = {
        let mut buffer = [0; 256];
        buffer.copy_from_slice(&gab.to_bytes_be());
        AuthKey::from_bytes(buffer)
    };

    let new_nonce_hash = auth_key.calc_new_nonce_hash(&new_nonce, nonce_number);
    let dh_hash = dh_gen.new_nonce_hash1;

    if dh_hash != new_nonce_hash {
        return Err(AuthKeyGenError::BadNonceHash);
    }

    Ok((auth_key, time_offset))
}

/// Find the RSA key's `(n, e)` pair for a certain fingerprint.
fn key_for_fingerprint(fingerprint: i64) -> Option<(BigUint, BigUint)> {
    // TODO Use a proper rsa module to parse the BEGIN RSA PUBLIC KEY
    //      instead of hardcoding their fingerprints and components.
    Some(match fingerprint {
        // New
        847625836280919973 => (BigUint::parse_bytes(b"22081946531037833540524260580660774032207476521197121128740358761486364763467087828766873972338019078976854986531076484772771735399701424566177039926855356719497736439289455286277202113900509554266057302466528985253648318314129246825219640197356165626774276930672688973278712614800066037531599375044750753580126415613086372604312320014358994394131667022861767539879232149461579922316489532682165746762569651763794500923643656753278887871955676253526661694459370047843286685859688756429293184148202379356802488805862746046071921830921840273062124571073336369210703400985851431491295910187179045081526826572515473914151", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        1562291298945373506 => (BigUint::parse_bytes(b"23978758553106631992002580305620005835060400692492410830911253690968985161770919571023213268734637655796435779238577529598157303153929847488434262037216243092374262144086701552588446162198373312512977891135864544907383666560742498178155572733831904785232310227644261688873841336264291123806158164086416723396618993440700301670694812377102225720438542027067699276781356881649272759102712053106917756470596037969358935162126553921536961079884698448464480018715128825516337818216719699963463996161433765618041475321701550049005950467552064133935768219696743607832667385715968297285043180567281391541729832333512747963903", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        -5859577972006586033 => (BigUint::parse_bytes(b"22718646979021445086805300267873836551952264292680929983215333222894263271262525404635917732844879510479026727119219632282263022986926715926905675829369119276087034208478103497496557160062032769614235480480336458978483235018994623019124956728706285653879392359295937777480998285327855536342942377483433941973435757959758939732133845114873967169906896837881767555178893700532356888631557478214225236142802178882405660867509208028117895779092487773043163348085906022471454630364430126878252139917614178636934412103623869072904053827933244809215364242885476208852061471203189128281292392955960922615335169478055469443233", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        6491968696586960280 => (BigUint::parse_bytes(b"24037766801008650742980770419085067708599000106468359115503808361335510549334399420739246345211161442047800836519033544747025851693968269285475039555231773313724462564908666239840898204833183290939296455776367417572678362602041185421910456164281750840651140599266716366431221860463163678044675384797103831824697137394559208723253047225996994374103488753637228569081911062604259973219466527532055001206549020539767836549715548081391829906556645384762696840019083743214331245456023666332360278739093925808884746079174665122518196162846505196334513910135812480878181576802670132412681595747104670774040613733524133809153", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),

        // Old
        -4344800451088585951 => (BigUint::parse_bytes(b"24403446649145068056824081744112065346446136066297307473868293895086332508101251964919587745984311372853053253457835208829824428441874946556659953519213382748319518214765985662663680818277989736779506318868003755216402538945900388706898101286548187286716959100102939636333452457308619454821845196109544157601096359148241435922125602449263164512290854366930013825808102403072317738266383237191313714482187326643144603633877219028262697593882410403273959074350849923041765639673335775605842311578109726403165298875058941765362622936097839775380070572921007586266115476975819175319995527916042178582540628652481530373407", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        -7306692244673891685 => (BigUint::parse_bytes(b"25081407810410225030931722734886059247598515157516470397242545867550116598436968553551465554653745201634977779380884774534457386795922003815072071558370597290368737862981871277312823942822144802509055492512145589734772907225259038113414940384446493111736999668652848440655603157665903721517224934142301456312994547591626081517162758808439979745328030376796953660042629868902013177751703385501412640560275067171555763725421377065095231095517201241069856888933358280729674273422117201596511978645878544308102076746465468955910659145532699238576978901011112475698963666091510778777356966351191806495199073754705289253783", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        -5738946642031285640 => (BigUint::parse_bytes(b"22347337644621997830323797217583448833849627595286505527328214795712874535417149457567295215523199212899872122674023936713124024124676488204889357563104452250187725437815819680799441376434162907889288526863223004380906766451781702435861040049293189979755757428366240570457372226323943522935844086838355728767565415115131238950994049041950699006558441163206523696546297006014416576123345545601004508537089192869558480948139679182328810531942418921113328804749485349441503927570568778905918696883174575510385552845625481490900659718413892216221539684717773483326240872061786759868040623935592404144262688161923519030977", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),
        8205599988028290019 => (BigUint::parse_bytes(b"24573455207957565047870011785254215390918912369814947541785386299516827003508659346069416840622922416779652050319196701077275060353178142796963682024347858398319926119639265555410256455471016400261630917813337515247954638555325280392998950756512879748873422896798579889820248358636937659872379948616822902110696986481638776226860777480684653756042166610633513404129518040549077551227082262066602286208338952016035637334787564972991208252928951876463555456715923743181359826124083963758009484867346318483872552977652588089928761806897223231500970500186019991032176060579816348322451864584743414550721639495547636008351", 10).unwrap(), BigUint::parse_bytes(b"65537", 10).unwrap()),

        _ => return None
    })
}

/// Encrypt the given data using RSA.
fn rsa_encrypt(data: &[u8], n: BigUint, e: BigUint, random_padding: bool) -> Vec<u8> {
    // Sha1::digest's len is always 20, we're left with 255 - 20 - x padding.
    let to_encrypt = {
        // sha1
        let mut buffer = Vec::with_capacity(255);
        let mut hasher = Sha1::new();
        hasher.update(data);
        buffer.extend(&hasher.digest().bytes());

        // + data
        buffer.extend(data);

        // + padding
        let mut random = vec![0; 255 - 20 - data.len()];
        if random_padding {
            getrandom(&mut random).expect("failed to get random data to encrypt with rsa");
        }
        buffer.extend(&random);

        buffer
    };

    let payload = BigUint::from_bytes_be(&to_encrypt);
    let encrypted = payload.modpow(&e, &n);
    let mut block = encrypted.to_bytes_be();
    while block.len() < 256 {
        block.insert(0, 0);
    }

    block
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
    let c = big(2 * pq / 4);
    let m = big(3 * pq / 4);
    let mut g = big(1u64);
    let mut r = big(1u64);
    let mut q = big(1u64);
    let mut x = big(0u64);
    let mut ys = big(0u64);
    let pq = big(pq);

    while g == big(1) {
        x = y.clone();
        for _ in 0..small(&r) {
            y = (y.modpow(&big(2), &pq) + &c) % &pq;
        }

        let mut k = big(0);
        while k < r && g == big(1) {
            ys = y.clone();
            for _ in 0..min(&m, &(&r - &k)) {
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

    #[test]
    fn test_rsa_encryption() {
        let (n, e) = key_for_fingerprint(847625836280919973).unwrap();
        let result = rsa_encrypt(b"Hello!", n, e, false);
        assert_eq!(
            result,
            vec![
                117, 112, 45, 76, 136, 210, 155, 106, 185, 52, 53, 81, 36, 221, 40, 217, 182, 42,
                71, 85, 136, 65, 200, 3, 20, 80, 247, 73, 155, 28, 156, 107, 211, 157, 39, 193, 88,
                28, 81, 52, 78, 81, 193, 121, 35, 112, 100, 167, 35, 174, 147, 157, 90, 195, 80,
                20, 253, 139, 79, 226, 79, 117, 227, 17, 92, 50, 161, 99, 105, 238, 43, 55, 58, 97,
                236, 148, 70, 185, 43, 46, 61, 240, 118, 24, 219, 10, 138, 253, 169, 153, 182, 112,
                43, 50, 181, 129, 155, 214, 234, 73, 112, 251, 52, 124, 168, 74, 96, 208, 195, 138,
                183, 12, 102, 229, 237, 1, 64, 68, 136, 137, 163, 184, 130, 238, 165, 51, 186, 208,
                94, 250, 32, 69, 237, 167, 23, 18, 60, 65, 74, 191, 222, 212, 62, 30, 180, 131,
                160, 73, 120, 110, 245, 3, 27, 18, 213, 26, 63, 247, 236, 183, 216, 4, 212, 65, 53,
                148, 95, 152, 247, 90, 74, 108, 241, 161, 223, 55, 85, 158, 48, 187, 233, 42, 75,
                121, 102, 195, 79, 7, 56, 230, 209, 48, 89, 133, 119, 109, 38, 223, 171, 124, 15,
                223, 215, 236, 32, 44, 199, 140, 84, 207, 130, 172, 35, 134, 199, 157, 14, 25, 117,
                128, 164, 250, 148, 48, 10, 35, 130, 249, 225, 22, 254, 130, 223, 155, 216, 114,
                229, 185, 218, 123, 66, 98, 35, 191, 26, 216, 88, 137, 48, 181, 30, 22, 93, 108,
                221, 2
            ]
        );
    }
}
