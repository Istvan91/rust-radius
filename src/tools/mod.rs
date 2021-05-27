//! Various helper functions, that are used by RADIUS Client & Server to encode/decode information
//! inside RADIUS packet
//! They are also available to crate users to prepare data before it is packed into RADIUS packet


use crypto::digest::Digest;
use crypto::md5::Md5;

use std::str::FromStr;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::convert::TryInto;

use crate::protocol::error::RadiusError;


/// Converts IPv6 Address string into vector of bytes
///
/// Should be used for any Attribute of type **ipv6addr** or **ipv6prefix** to ensure value is encoded correctly
pub fn ipv6_string_to_bytes(ipv6: &str) -> Result<Vec<u8>, RadiusError> {
    let mut parsed_ipv6 = ipv6.split("/");
    let mut bytes: Vec<u8>     = Vec::with_capacity(18);
    let ipv6_address           = Ipv6Addr::from_str(parsed_ipv6.next().unwrap()).map_err(|err| RadiusError::MalformedIpAddrError { error: err.to_string() })?;

    if let Some(netmask) = parsed_ipv6.next() {
        bytes.extend(&u16_to_be_bytes(netmask.parse::<u16>().map_err(|err| RadiusError::MalformedIpAddrError {error: err.to_string()})?));
    }
    bytes.extend(&ipv6_address.octets());
    Ok(bytes)
}

/// Converts IPv6 bytes into IPv6 string
pub fn bytes_to_ipv6_string(ipv6: &[u8]) -> Result<String, RadiusError> {
    if ipv6.len() == 18 {
        // Case with subnet
        let subnet = u16_from_be_bytes(&ipv6[0..2]);
        let ipv6 = Ipv6Addr::new(
            u16_from_be_bytes(&ipv6[2..4]),
            u16_from_be_bytes(&ipv6[4..6]),
            u16_from_be_bytes(&ipv6[6..8]),
            u16_from_be_bytes(&ipv6[8..10]),
            u16_from_be_bytes(&ipv6[10..12]),
            u16_from_be_bytes(&ipv6[12..14]),
            u16_from_be_bytes(&ipv6[14..16]),
            u16_from_be_bytes(&ipv6[16..]),
            );
        Ok(format!("{}/{}",ipv6, subnet))
    } else {
        // Case without subnet
        Ok(Ipv6Addr::new(
            u16_from_be_bytes(&ipv6[0..2]),
            u16_from_be_bytes(&ipv6[2..4]),
            u16_from_be_bytes(&ipv6[4..6]),
            u16_from_be_bytes(&ipv6[6..8]),
            u16_from_be_bytes(&ipv6[8..10]),
            u16_from_be_bytes(&ipv6[10..12]),
            u16_from_be_bytes(&ipv6[12..14]),
            u16_from_be_bytes(&ipv6[14..]),
            ).to_string())
    }
}

/// Converts IPv4 Address string into vector of bytes
///
/// Should be used for any Attribute of type **ipaddr** or **ipv4prefix** to ensure value is encoded correctly
pub fn ipv4_string_to_bytes(ipv4: &str) -> Result<Vec<u8>, RadiusError> {
    let mut splitted = ipv4.split("/");
    let ipv4 = splitted.next().unwrap().parse::<Ipv4Addr>().map_err(|err| RadiusError::MalformedIpAddrError { error: err.to_string() })?;
    let mut bytes: Vec<u8> = Vec::with_capacity(6);
    if let Some(netmask) = splitted.next() {
        bytes.push(0);
        bytes.push(netmask.parse::<u8>().map_err(|err| RadiusError::MalformedIpAddrError { error: err.to_string() })?);
    }
    bytes.extend_from_slice(&ipv4.octets());

    Ok(bytes)
}

/// Converts IPv4 bytes into IPv4 string
pub fn bytes_to_ipv4_string(ipv4: &[u8]) -> Result<String, RadiusError> {
    let mut buf = [0u8; 4];
    Ok(match ipv4.len() {
        4 => {
            buf.copy_from_slice(ipv4);
            Ipv4Addr::from(buf).to_string()
        }
        6 => {
            let netmask = u8::from(ipv4[1]);
            buf.copy_from_slice(&ipv4[2..]);
            format!("{}/{}", Ipv4Addr::from(buf), netmask)
        }
        _ => {
            return Err(RadiusError::MalformedIpAddrError { error: format!("Maleformed IPv4 {:?}", ipv4)});
        }
    })
}

/// Converts u32 into vector of bytes
///
/// Should be used for any Attribute of type **integer** to ensure value is encoded correctly
pub fn integer_to_bytes(integer: u32) -> Vec<u8> {
    integer.to_be_bytes().to_vec()
}

/// Converts integer bytes into u32
pub fn bytes_to_integer(integer: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*integer)
}

/// Converts u64 into vector of bytes
///
/// Should be used for any Attribute of type **integer64** to ensure value is encoded correctly
pub fn integer64_to_bytes(integer: u64) -> Vec<u8> {
    integer.to_be_bytes().to_vec()
}

/// Converts integer bytes into u64
pub fn bytes_to_integer64(integer: &[u8; 8]) -> u64 {
    u64::from_be_bytes(*integer)
}

/// Converts timestamp (u64) into vector of bytes
///
/// Should be used for any Attribute of type **date** to ensure value is encoded correctly
pub fn timestamp_to_bytes(timestamp: u64) -> Vec<u8> {
    timestamp.to_be_bytes().to_vec()
}

/// Converts timestamp bytes into u64
pub fn bytes_to_timestamp(timestamp: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*timestamp)
}


/// Encrypts data since RADIUS packet is sent in plain text
///
/// Should be used to encrypt value of **User-Password** attribute (but could also be used to
/// encrypt any data)
pub fn encrypt_data(data: &[u8], authenticator: &[u8], secret: &[u8]) -> Vec<u8> {
    /* Step 1. Ensure that data buffer's length is multiple of 16
    *  Step 2. Construct hash:
    *
    *  On each iteration:
    *   1. read 16 elements from data
    *   2. calculate MD5 hash for: provided secret + (authenticator(on 1st iteration) or 16 elements of result from previous iteration (2nd+ iteration))
    *   3. execute bitwise XOR between each of 16 elements of MD5 hash and data buffer and record it in results vector
    *
    * Step 3. Return result vector
    */
    let mut hash = [0u8; 16];
    let padding  = 16 - data.len() % 16;

    let mut result = Vec::with_capacity(data.len() + padding);
    result.extend_from_slice(data);
    result.extend_from_slice(&hash[..padding]);

    let prev_result = authenticator;
    let current     = result.as_mut_slice();

    encrypt_helper(current, prev_result, &mut hash, secret);

    result
}

/// Decrypts data since RADIUS packet is sent in plain text
///
/// Should be used to decrypt value of **User-Password** attribute (but could also be used to
/// decrypt any data)
pub fn decrypt_data(data: &[u8], authenticator: &[u8], secret: &[u8]) -> Vec<u8> {
    /*
     * To decrypt the data, we need to apply the same algorithm as in encrypt_data()
     * but with small change
     *
     *  On each iteration:
     *   1. read 16 elements from data
     *   2. calculate MD5 hash for: provided secret + (authenticator(on 1st iteration) or 16 elements of data buffer from previous iteration (2nd+ iteration))
     *   3. execute bitwise XOR between each of 16 elements of MD5 hash and data buffer and record it in results vector
     *
     */
    let mut result      = Vec::with_capacity(data.len());
    let mut prev_result = authenticator;
    let mut hash        = [0u8; 16];

    for data_chunk in data.chunks_exact(16) {
        let mut md5  = Md5::new();
        md5.input(secret);
        md5.input(prev_result);
        md5.result(&mut hash);

        for (_data, _hash) in data_chunk.iter().zip(hash.iter_mut()) {
            *_hash ^= _data
        }

        result.extend_from_slice(&hash);
        prev_result = data_chunk;
    }

    while result[result.len()-1] == 0 {
        result.pop();
    }

    result
}

/// Encrypts data with salt since RADIUS packet is sent in plain text
///
/// Should be used for RADIUS Tunnel-Password Attribute
pub fn salt_encrypt_data(data: &[u8], authenticator: &[u8], salt: &[u8], secret: &[u8]) -> Vec<u8> {
    if data.len() == 0 {
        return Vec::new();
    }

    // let salt       = &data[..2];
    let mut hash   = [0u8; 16];
    let padding    = 15 - data.len() % 16;
    let mut result = Vec::with_capacity(data.len() + 3 + padding); // make buffer big enough to fit the salt & encrypted data

    result.extend_from_slice(salt);
    result.push(data.len() as u8);
    result.extend_from_slice(data);
    result.extend_from_slice(&hash[..padding]);

    let salted_authenticator = &mut [0u8; 18];
    salted_authenticator[..16].copy_from_slice(authenticator);
    salted_authenticator[16..].copy_from_slice(salt);

    let prev_result = &salted_authenticator[..];
    let current     = &mut result[2..];

    encrypt_helper(current, prev_result, &mut hash, secret);

    result
}

/// Decrypts data with salt since RADIUS packet is sent in plain text
///
/// Should be used for RADIUS Tunnel-Password Attribute
pub fn salt_decrypt_data(data: &[u8], authenticator: &[u8], secret: &[u8]) -> Result<Vec<u8>, RadiusError> {
    /*
     * The salt decryption behaves almost the same as normal Password encryption in RADIUS
     * The main difference is the presence of a two byte salt, which is appended to the authenticator
     */
    if data.len() <= 1 {
        return Err(RadiusError::MalformedAttributeError {error: "salt encrypted attribute too short".to_string()});
    }
    if data.len() <= 3 {
        // There is a Salt or there is a salt & data.len(): Both cases mean "Password is empty"
        return Ok(Vec::new());
    }

    let salted_authenticator = &mut [0u8; 18];
    salted_authenticator[..16].copy_from_slice(authenticator);
    salted_authenticator[16..].copy_from_slice(&data[..2]);

    let mut hash        = [0u8; 16];
    let mut result      = Vec::with_capacity(data.len()-2);
    let mut prev_result = &salted_authenticator[..];

    for data_chunk in (&data[2..]).chunks_exact(16) {
        let mut md5 = Md5::new();
        md5.input(secret);
        md5.input(prev_result);
        md5.result(&mut hash);


        for (_data, _hash) in data_chunk.iter().zip(hash.iter_mut()) {
            *_hash ^= _data
        }
        result.extend_from_slice(&hash);

        prev_result = data_chunk;
    }

    let target_len = usize::from(result.remove(0));

    if target_len > data.len() - 3 {
        return Err(RadiusError::MalformedAttributeError { error: "Tunnel Password is too long (shared secret might be wrong)".to_string()});
    }

    result.truncate(target_len);
    Ok(result)
}

// -----------------------------------------
fn encrypt_helper<'a:'b, 'b>(mut data: &'a mut [u8], mut result: &'b [u8], mut hash: &mut[u8], secret: &[u8]) {
    loop {
        let mut md5 = Md5::new();
        md5.input(secret);
        md5.input(result);
        md5.result(&mut hash);

        for (_data, _hash) in data.iter_mut().zip(hash.iter()) {
            *_data ^= _hash
        }

        let (_prev, _current) = data.split_at_mut(16);
        result = _prev;
        data   = _current;

        if data.len() == 0 { break }
    }
}

// WIP
// fn decrypt_helper<'a:'b, 'b>(data: &'a mut [u8], mut prev_result: &'b [u8], result: &mut Vec<u8>, mut hash: &mut[u8], secret: &[u8]) {
//     for data_chunk in data.chunks_exact(16) {
//         let mut md5  = Md5::new();
//         md5.input(secret);
//         md5.input(prev_result);
//         md5.result(&mut hash);

//         for (_data, _hash) in data_chunk.iter().zip(hash.iter_mut()) {
//             *_hash ^= _data
//         }

//         result.extend_from_slice(&hash);
//         prev_result = data_chunk;
//     }
// }

fn u16_to_be_bytes(u16_data: u16) -> [u8;2] {
    u16_data.to_be_bytes()
}

fn u16_from_be_bytes(bytes: &[u8]) -> u16 {
    u16::from_be_bytes(bytes.try_into().expect("slice with incorrect length"))
}
// -----------------------------------------


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv6_to_bytes_wo_subnet() {
        let ipv6_bytes = ipv6_string_to_bytes("fc66::1").unwrap();
        assert_eq!(ipv6_bytes, vec![252, 102, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }
    #[test]
    fn test_bytes_to_ipv6_string_wo_subnet() {
        let expected_ipv6_string = "fc66::1";
        let ipv6_bytes           = vec![252, 102, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        assert_eq!(expected_ipv6_string, bytes_to_ipv6_string(&ipv6_bytes).unwrap());
    }

    #[test]
    fn test_ipv6_to_bytes_w_subnet() {
        let ipv6_bytes = ipv6_string_to_bytes("fc66::1/64").unwrap();
        assert_eq!(ipv6_bytes, [0, 64, 252, 102, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    }
    #[test]
    fn test_bytes_to_ipv6_string_w_subnet() {
        let expected_ipv6_string = "fc66::1/64";
        let ipv6_bytes           = vec![0, 64, 252, 102, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];

        assert_eq!(expected_ipv6_string, bytes_to_ipv6_string(&ipv6_bytes).unwrap());
    }

    #[test]
    fn test_ipv4_string_to_bytes() {
        let ipv4_bytes = ipv4_string_to_bytes("192.1.10.1").unwrap();

        assert_eq!(ipv4_bytes, [192, 1, 10, 1]);
    }

    #[test]
    fn test_ipv4_string_to_bytes_w_subnet() {
        let ipv4_bytes = ipv4_string_to_bytes("192.1.10.0/30").unwrap();

        assert_eq!(ipv4_bytes, [0, 30, 192, 1, 10, 0]);
    }

    #[test]
    fn test_ipv4_bytes_to_string() {
        let ipv4_bytes = vec![192, 1, 10, 1];
        let ipv4_string = bytes_to_ipv4_string(&ipv4_bytes).unwrap();

        assert_eq!(ipv4_string, "192.1.10.1".to_string());
    }

    #[test]
    fn test_ipv4_w_subnet_bytes_to_string() {
        let ipv4_bytes = vec![0, 30, 192, 1, 10, 0];
        let ipv4_string = bytes_to_ipv4_string(&ipv4_bytes).unwrap();

        assert_eq!(ipv4_string, "192.1.10.0/30".to_string());
    }

    #[test]
    fn test_encrypt_data() {
        let secret        = String::from("secret");
        let authenticator = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let encrypted_bytes = encrypt_data("password".as_bytes(), &authenticator, &secret.as_bytes());

        assert_eq!(encrypted_bytes, vec![135, 116, 155, 239, 226, 89, 90, 221, 62, 29, 218, 130, 102, 174, 191, 250]);
    }

    #[test]
    fn test_encrypt_data_long() {
        let secret        = String::from("secret");
        let authenticator = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let encrypted_bytes = encrypt_data("a very long password, which will need multiple iterations".as_bytes(), &authenticator, &secret.as_bytes());
        assert_eq!(encrypted_bytes, vec![150, 53, 158, 249, 231, 79, 8, 213, 81, 115, 189, 162, 22, 207, 204, 137, 193,
                   149, 82, 147, 72, 149, 79, 48, 187, 199, 194, 200, 246, 6, 186, 182, 220, 19, 227, 32, 26, 20, 9, 152,
                   63, 40, 41, 91, 212, 22, 158, 54, 91, 247, 151, 67, 250,170, 105, 94, 20, 105, 120, 196, 237, 191, 99, 69]
        );
    }

    #[test]
    fn test_encrypt_data_limit_long() {
        let secret        = String::from("secret");
        let authenticator = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let data          = "a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long passw";

        let encrypted_bytes = encrypt_data(data.as_bytes(), &authenticator, &secret.as_bytes());
        assert_eq!(encrypted_bytes, vec![150, 53, 158, 249, 231, 79, 8, 213, 81, 115, 189, 162, 22, 207, 204, 137, 193, 149, 82, 147, 72, 149, 79, 48, 187, 199, 194, 200,
                                         246, 6, 186, 182, 220, 19, 227, 32, 26, 20, 9, 152, 63, 40, 41, 91, 212, 22, 158, 54, 91, 247, 151, 67, 250, 170, 105, 94, 20, 71,
                                         88, 165, 205, 201, 6, 55, 222, 205, 192, 227, 172, 93, 166, 15, 33, 86, 56, 181, 52, 4, 49, 190, 186, 17, 125, 50, 140, 52, 130, 194,
                                         125, 93, 177, 65, 217, 195, 23, 75, 175, 219, 244, 156, 133, 145, 20, 176, 36, 90, 16, 77, 148, 221, 251, 155, 9, 107, 213, 140, 107,
                                         112, 161, 99, 6, 108, 106, 33, 69, 192, 191, 98, 30, 147, 197, 72, 160, 234, 50, 243, 195, 62, 72, 225, 19, 63, 28, 221, 164, 43, 67,
                                         63, 206, 208, 124, 254, 202, 118, 229, 58, 180, 210, 100, 149, 120, 97, 23, 203, 197, 139, 244, 241, 175, 232, 149, 77, 43, 231, 27, 56,
                                         250, 58, 251, 6, 203, 197, 190, 78, 83, 127, 164, 31, 211, 52, 74, 92, 36, 250, 236, 210, 72, 52, 55, 248, 161, 160, 95, 102, 63, 190, 43,
                                         253, 224, 114, 62, 23, 11, 242, 186, 91, 132, 14, 76, 171, 26, 1, 51, 78, 144, 50, 228, 212, 47, 104, 98, 60, 245, 1, 103, 217, 49, 105,
                                         38, 108, 93, 85, 224, 227, 33, 50, 144, 0, 233, 54, 174, 67, 174, 101, 189, 41]);
    }

    #[test]
    fn test_decrypt_data() {
        let secret         = String::from("secret");
        let authenticator  = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let expected_data  = String::from("password");
        let encrypted_data = vec![135, 116, 155, 239, 226, 89, 90, 221, 62, 29, 218, 130, 102, 174, 191, 250];

        let decrypted_data = decrypt_data(&encrypted_data, &authenticator, &secret.as_bytes());

        assert_eq!(expected_data.as_bytes().to_vec(), decrypted_data);
    }

    #[test]
    fn test_descrypt_data_long() {
        let secret         = String::from("secret");
        let authenticator  = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let expected_data  = String::from("a very long password, which will need multiple iterations");
        let encrypted_data = vec![150, 53, 158, 249, 231, 79, 8, 213, 81, 115, 189, 162, 22, 207, 204, 137, 193,
        149, 82, 147, 72, 149, 79, 48, 187, 199, 194, 200, 246, 6, 186, 182, 220, 19, 227, 32, 26, 20, 9, 152, 63,
        40, 41, 91, 212, 22, 158, 54, 91, 247, 151, 67, 250,170, 105, 94, 20, 105, 120, 196, 237, 191, 99, 69];

        let decrypted_data = decrypt_data(&encrypted_data, &authenticator, &secret.as_bytes());
        assert_eq!(expected_data.as_bytes().to_vec(), decrypted_data);
    }

    #[test]
    fn test_descrypt_data_limit_long() {
        let secret         = String::from("secret");
        let authenticator  = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];

        let expected_data  = String::from("a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long password, which will need multiple iterations. a very long passw");
        let encrypted_data = vec![150, 53, 158, 249, 231, 79, 8, 213, 81, 115, 189, 162, 22, 207, 204, 137, 193, 149, 82, 147, 72, 149, 79, 48, 187, 199, 194, 200,
                                  246, 6, 186, 182, 220, 19, 227, 32, 26, 20, 9, 152, 63, 40, 41, 91, 212, 22, 158, 54, 91, 247, 151, 67, 250, 170, 105, 94, 20, 71,
                                  88, 165, 205, 201, 6, 55, 222, 205, 192, 227, 172, 93, 166, 15, 33, 86, 56, 181, 52, 4, 49, 190, 186, 17, 125, 50, 140, 52, 130, 194,
                                  125, 93, 177, 65, 217, 195, 23, 75, 175, 219, 244, 156, 133, 145, 20, 176, 36, 90, 16, 77, 148, 221, 251, 155, 9, 107, 213, 140, 107,
                                  112, 161, 99, 6, 108, 106, 33, 69, 192, 191, 98, 30, 147, 197, 72, 160, 234, 50, 243, 195, 62, 72, 225, 19, 63, 28, 221, 164, 43, 67,
                                  63, 206, 208, 124, 254, 202, 118, 229, 58, 180, 210, 100, 149, 120, 97, 23, 203, 197, 139, 244, 241, 175, 232, 149, 77, 43, 231, 27, 56,
                                  250, 58, 251, 6, 203, 197, 190, 78, 83, 127, 164, 31, 211, 52, 74, 92, 36, 250, 236, 210, 72, 52, 55, 248, 161, 160, 95, 102, 63, 190, 43,
                                  253, 224, 114, 62, 23, 11, 242, 186, 91, 132, 14, 76, 171, 26, 1, 51, 78, 144, 50, 228, 212, 47, 104, 98, 60, 245, 1, 103, 217, 49, 105,
                                  38, 108, 93, 85, 224, 227, 33, 50, 144, 0, 233, 54, 174, 67, 174, 101, 189, 41];

        let decrypted_data = decrypt_data(&encrypted_data, &authenticator, &secret.as_bytes());
        assert_eq!(expected_data.as_bytes().to_vec(), decrypted_data);
    }

    #[test]
    fn test_salt_encrypt_data() {
        let secret               = b"secret";
        let authenticator: &[u8] = &[0u8; 16];

        let plaintext             = b"password";
        let encrypted_data: &[u8] = &[0x85, 0x9a, 0xe3, 0x88, 0x34, 0x49, 0xf2, 0x1e, 0x14, 0x4c, 0x76, 0xc8, 0xb2, 0x1a, 0x1d, 0x4f, 0x0c, 0xdc];
        let salt                  = &encrypted_data[..2];

        assert_eq!(encrypted_data, salt_encrypt_data(plaintext, authenticator, salt, secret).as_slice());
    }

    #[test]
    fn test_salt_encrypt_data_long() {
        let secret               = b"secret";
        let authenticator: &[u8] = &[0u8; 16];

        let plaintext_long             = b"a very long password, which will need multiple iterations";
        let encrypted_data_long: &[u8] = &[0x85, 0xd9, 0x61, 0x72, 0x75, 0x37, 0xcf, 0x15, 0x20,
        0x19, 0x3b, 0x38, 0x39, 0x0e, 0x42, 0x21, 0x9b, 0x5e, 0xcb, 0x93, 0x25, 0x7d, 0xb4, 0x07,
        0x0c, 0xc1, 0x52, 0xcf, 0x38, 0x76, 0x29, 0x02, 0xc7, 0xb1, 0x29, 0xdf, 0x63, 0x96, 0x26,
        0x1a, 0x27, 0xe5, 0xc3, 0x13, 0x78, 0xa7, 0x97, 0xd8, 0x97, 0x9a, 0x45, 0xc3, 0x70, 0xd3,
        0xe4, 0xe2, 0xae, 0xd0, 0x55, 0x77, 0x19, 0xa5, 0xb6, 0x44, 0xe6, 0x8a];
        let salt                       = &encrypted_data_long[..2];

        assert_eq!(encrypted_data_long, salt_encrypt_data(plaintext_long, authenticator, salt, secret).as_slice());
    }

    #[test]
    fn test_salt_decrypt_data() {
        let secret               = b"secret";
        let authenticator: &[u8] = &[0u8; 16];

        let plaintext: &[u8]      = b"password";
        let encrypted_data: &[u8] = &[0x85, 0x9a, 0xe3, 0x88, 0x34, 0x49, 0xf2, 0x1e, 0x14, 0x4c, 0x76, 0xc8, 0xb2, 0x1a, 0x1d, 0x4f, 0x0c, 0xdc];

        assert_eq!(plaintext, salt_decrypt_data(encrypted_data, authenticator, secret).unwrap().as_slice());
    }

    #[test]
    fn test_salt_decrypt_data_long() {
        let secret               = b"secret";
        let authenticator: &[u8] = &[0u8; 16];

        let plaintext_long             = b"a very long password, which will need multiple iterations";
        let encrypted_data_long: &[u8] = &[0x85, 0xd9, 0x61, 0x72, 0x75, 0x37, 0xcf, 0x15, 0x20,
        0x19, 0x3b, 0x38, 0x39, 0x0e, 0x42, 0x21, 0x9b, 0x5e, 0xcb, 0x93, 0x25, 0x7d, 0xb4, 0x07,
        0x0c, 0xc1, 0x52, 0xcf, 0x38, 0x76, 0x29, 0x02, 0xc7, 0xb1, 0x29, 0xdf, 0x63, 0x96, 0x26,
        0x1a, 0x27, 0xe5, 0xc3, 0x13, 0x78, 0xa7, 0x97, 0xd8, 0x97, 0x9a, 0x45, 0xc3, 0x70, 0xd3,
        0xe4, 0xe2, 0xae, 0xd0, 0x55, 0x77, 0x19, 0xa5, 0xb6, 0x44, 0xe6, 0x8a];

        assert_eq!(plaintext_long.to_vec(), salt_decrypt_data(encrypted_data_long, authenticator, secret).unwrap());
    }

    #[test]
    fn test_integer_to_bytes() {
        let integer: u32 = 10000;

        assert_eq!(vec![0, 0, 39, 16], integer_to_bytes(integer));
    }

    #[test]
    fn test_bytes_to_integer() {
        let integer_bytes = [0, 0, 39, 16];

        assert_eq!(10000, bytes_to_integer(&integer_bytes));
    }

    #[test]
    fn test_timestamp_to_bytes() {
        let timestamp: u64 = 1598523933;

        assert_eq!(vec![0, 0, 0, 0, 95, 71, 138, 29], timestamp_to_bytes(timestamp));
    }

    #[test]
    fn test_bytes_to_timestamp() {
        let timestamp_bytes = [95, 71, 138, 29];

        assert_eq!(1598523933, bytes_to_timestamp(&timestamp_bytes));
    }
}
