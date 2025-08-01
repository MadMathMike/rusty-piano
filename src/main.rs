use blowfish::cipher::{BlockEncrypt, BlockSizeUser, KeyInit};
use blowfish::{Blowfish, BlowfishLE};

use cipher::BlockDecrypt;

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

    let auth_response = client
        .post(request_uri)
        .json(&partner_auth_body)
        .send()
        .expect("Error making auth call");

    // TODO: Error out on response status code != 200

    let auth_response = auth_response
        .json::<PartnerAuthResponse>()
        .expect("Failed to parse partner auth response");
    println!("{:?}", auth_response);

    // TODO: calculate sync time
    println!("syncTime: {:?}", &auth_response.result.sync_time);
    let decryption_key = "U#IO$RZPAB%VX2";

    let sync_time_decrypted_bytes = decode_and_decrypt(decryption_key, &auth_response.result.sync_time);
    
    for byte in sync_time_decrypted_bytes {
        println!("Byte: {:?}. Is ASCII: {}", byte, if byte.is_ascii() {String::from_utf8(vec![byte]).unwrap()} else { "meh".to_owned() });
    }

    //decrypted_sync_time_bytes[4..].iter().map(|b| )
    // let decrypted_sync_time = String::from_utf8(sync_time_decrypted_bytes[4..].to_vec()).expect("Error decoding bytes as UTF8");
    //  println!("{:?}", decrypted_sync_time);

    let partner_id = &auth_response.result.partner_id;
    let request_uri = format!(
        "https://internal-tuner.pandora.com/services/json/?method=auth.userLogin&partner_id={partner_id}"
    );

    let user_auth_body = json!({
        "loginType": "user",
        "username": "MichaelPeterson27@live.com",
        "password": "todo",
        "partnerAuthToken": auth_response.result.partner_auth_token.clone()
    });

    let user_auth_body_as_str = user_auth_body.to_string();

    let encryption_key = "2%3WCL*JU$MP]4";
    let encrypted_body = encrypt(encryption_key, &user_auth_body_as_str);

    let auth_response = client
        .post(request_uri)
        .body(encrypted_body)
        .send()
        .expect("Error making user auth call");

    // println!("{}", auth_response.text().unwrap());

    let user_auth_response = auth_response
        .json::<PandoraResponse>()
        .expect("Failed to parse partner auth response");
    println!("{:?}", user_auth_response);

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

    hex::encode(&ciphertext)
}

fn decode_and_decrypt(key: &str, input: &str) -> Vec<u8> {
    let decoded_input: Vec<u8> = hex::decode(input).expect("Error hex decoding input string");

    decrypt(key, decoded_input)
}

fn decrypt(key: &str, input: Vec<u8>) -> Vec<u8> {
    let blowfish = Blowfish::<byteorder::BigEndian>::new_from_slice(key.as_bytes()).unwrap();
    let block_size = BlowfishLE::block_size(); // should be 8

    let mut plaintext = vec![0u8; input.len()];

    for (in_block, out_block) in input
        .chunks(block_size)
        .zip(plaintext.chunks_mut(block_size))
    {
        blowfish.decrypt_block_b2b(in_block.into(), out_block.into());
    }

    plaintext
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
        let decrypted = decode_and_decrypt("R=U!LH$O2B#", "4a6b45612b018614c92c50dc73462bbd");
        let decrypted = String::from_utf8(decrypted).unwrap();
        assert_eq!(decrypted, "è.<Ú1477631903");
    }
}
