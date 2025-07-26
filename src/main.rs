use blowfish::cipher::{BlockEncrypt, BlockSizeUser, KeyInit};
use blowfish::{Blowfish, BlowfishLE};

use cipher::BlockDecrypt;
use hex::FromHex;

use reqwest::blocking::Client;
use rodio::{Decoder, OutputStreamBuilder};
use serde::Deserialize;
use serde_json::json;
use std::fs::File;
use std::thread::sleep;

#[derive(Debug, Deserialize)]
struct PartnerAuthResult {
    #[serde(rename = "partnerId")]
    pub partner_id: String,
    #[serde(rename = "partnerAuthToken")]
    pub partner_auth_token: String,
    #[serde(rename = "syncTime")]
    pub sync_time: String,
}

#[derive(Debug, Deserialize)]
struct PartnerAuthResponse {
    pub stat: String,
    pub result: PartnerAuthResult,
}

#[derive(Debug, Deserialize)]
struct PandoraResponse {
    pub stat: String,
}

// pandora request failure body: {"stat":"fail","message":"An unexpected error occurred","code":6}

fn main() {
    let partner_auth_body = json!({
        "username": "pandora one",
        "password": "TVCKIBGS9AO9TSYLNNFUML0743LH82D",
        "deviceModel": "D01",
        "version": "5"
    });

    let request_uri = "https://internal-tuner.pandora.com/services/json/?method=auth.partnerLogin";

    // Send auth request
    let client = Client::new();

    let response = client
        .post(request_uri)
        .json(&partner_auth_body)
        .send()
        .expect("Error making auth call");

    println!("Partner auth status code: {:?}", response.status());

    // // TODO: Error out on response status code != 200

    let partner_auth_response = response
        .json::<PartnerAuthResponse>()
        .expect("Failed to parse partner auth response");
    println!("{:?}", partner_auth_response);

    // TODO: calculate sync time
    let key = "2%3WCL*JU$MP]4";

    let decrypted_sync_time_bytes = decrypt_as_bytes(key, &partner_auth_response.result.sync_time);
    println!("{:?}", decrypted_sync_time_bytes);

    //decrypted_sync_time_bytes[4..].iter().map(|b| )
    //let decrypted_sync_time = String::from_utf8(decrypted_sync_time_bytes[4..].to_vec()).expect("Error decoding bytes as UTF8");
    // println!("{:?}", decrypted_sync_time);

    let partner_id = &partner_auth_response.result.partner_id;
    let request_uri = format!(
        "https://internal-tuner.pandora.com/services/json/?method=auth.userLogin&partner_id={partner_id}"
    );

    let user_auth_body = json!({
        "loginType": "user",
        "username": "MichaelPeterson27@live.com",
        "password": "todo",
        "partnerAuthToken": partner_auth_response.result.partner_auth_token.clone()
    });

    let user_auth_body_as_str = user_auth_body.to_string();

    let encrypted_body = encrypt(key, &user_auth_body_as_str);

    let response = client
        .post(request_uri)
        .body(encrypted_body)
        .send()
        .expect("Error making user auth call");

    println!("User auth status code: {:?}", response.status());

    // let user_auth_response = response
    //     .json::<PandoraResponse>()
    //     .expect("Failed to parse partner auth response");
    // println!("{:?}", user_auth_response);

    println!("{}", response.text().unwrap());

    play_sample_sound();
}

fn encrypt(key: &str, input: &str) -> String {
    let blowfish = Blowfish::<byteorder::BigEndian>::new_from_slice(key.as_bytes()).unwrap();
    let block_size = BlowfishLE::block_size(); // should be 8

    let plaintext = input.as_bytes();
    println!("{:?}", plaintext);
    let padded_len = plaintext.len().div_ceil(block_size) * block_size;
    let mut padded_plaintext = vec![0u8; padded_len];
    padded_plaintext[..plaintext.len()].copy_from_slice(plaintext);

    let mut ciphertext = vec![0u8; padded_len];

    for (in_block, out_block) in padded_plaintext
        .chunks(block_size)
        .zip(ciphertext.chunks_mut(block_size))
    {
        blowfish.encrypt_block_b2b(in_block.into(), out_block.into());
    }

    hex_encode(&ciphertext)
}

fn decrypt(key: &str, input: &str) -> String {
    String::from_utf8(decrypt_as_bytes(key, input)).expect("Error converting bytes to UTF8")
}

fn decrypt_as_bytes(key: &str, input: &str) -> Vec<u8> {
    let decoded_input: Vec<u8> = hex::decode(input).expect("Error hex decoding input string");

    let blowfish = Blowfish::<byteorder::BigEndian>::new_from_slice(key.as_bytes()).unwrap();
    let block_size = BlowfishLE::block_size(); // should be 8

    let mut plaintext = vec![0u8; decoded_input.len()];

    for (in_block, out_block) in decoded_input
        .chunks(block_size)
        .zip(plaintext.chunks_mut(block_size))
    {
        blowfish.decrypt_block_b2b(in_block.into(), out_block.into());
    }

    plaintext
}

// TODO: replace with hex library
fn hex_encode(input: &[u8]) -> String {
    let mut output = String::with_capacity(input.len() * 2);
    for b in input {
        output.push_str(&format!("{:02x}", b));
    }
    output
}

fn play_sample_sound() {
    let stream_handle =
        OutputStreamBuilder::open_default_stream().expect("Error opening default audio stream");
    // TODO: What is this doing?
    // Note that playback stops when the sink is dropped
    let _sink = rodio::Sink::connect_new(stream_handle.mixer());
    let path = "file_example_MP3_2MG.mp3";
    let file = File::open(path).expect("Error opening file");
    let source = Decoder::try_from(file).expect("Error decoding file");
    // Play the sound directly on the device
    stream_handle.mixer().add(source);

    sleep(std::time::Duration::from_secs(5));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_test_vector() {
        let encrypted = encrypt("R=U!LH$O2B#", "è.<Ú1477631903");
        assert_eq!(encrypted, "4a6b45612b018614c92c50dc73462bbd");
    }

    #[test]
    fn decrypt_test_value() {
        let decrypted = decrypt("R=U!LH$O2B#", "4a6b45612b018614c92c50dc73462bbd");
        assert_eq!(decrypted, "è.<Ú1477631903");
    }

    #[test]
    fn test_hex_encode() {
        let input = "Hello world!";
        let expected_ouput = "48656c6c6f20776f726c6421";
        let output = hex_encode(input.as_bytes());

        assert_eq!(expected_ouput, output);
    }
}
