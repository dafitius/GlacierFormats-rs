use glacier_base::encryption::xtea::XteaError;
/// Implementation of XTEA encryption and decryption methods.
pub struct Xtea;

impl Xtea {

    /// Checks if a given buffer represents an encrypted text file.
    /// This function will check for the presence of a default header in the text file.
    pub fn is_encrypted_text_file(input_buffer: &[u8]) -> bool {
        glacier_base::encryption::xtea::Xtea::is_encrypted_text_file(input_buffer)
    }

    /// Decrypts a text file given its buffer, uses the default xtea key.
    pub fn decrypt_text_file(input_buffer: &[u8]) -> Result<String, XteaError> {
        glacier_base::encryption::xtea::Xtea::decrypt_text_file(input_buffer)
    }

    pub fn encrypt_text_file(input_string: String) -> Result<Vec<u8>, XteaError> {
        glacier_base::encryption::xtea::Xtea::encrypt_text_file(input_string)
    }
}
