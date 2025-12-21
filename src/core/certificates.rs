use crate::core::adb::{ACommand as AdbCommand};
use std::fs::{self, File};
use std::io::{self, BufReader};
use zip::ZipArchive;
use std::process::Command;

/* CRATE REFERING TO CERTIFICATES AND THEIR RESPECTIVE FUNCTIONS

    despite what the Certificate might ought you to believe, it is important to mention the following facts
    -any Vendor under the Google name is able to utilize the certificates to repackage their own apks
    -the only security assurance we have is under Google supervision of their vendors and whatever validity methods they may use to verify apk safety
    -phones from smaller companies might not be able to get hands on the official Google name certificates
    All this leads to the conclusion that certificates exist as a method of prevention and awareness over any actual real security,
    there is no certificate(haha funny) of whether an apk is not malicious.
*/

#[derive(Clone, Debug)]
pub enum CertificateState {
    UnmodifiedCertState,
    ModifiedCertState,
    UnknownCertState,
}


pub fn match_certificate(certificate: CertificateState) -> String {
    match certificate {
        CertificateState::UnmodifiedCertState => "\n\nunmodified apk.".to_string(),
        CertificateState::ModifiedCertState => "\n\nmodified apk. WARNING: either the apk has been modified by a non-vendor issuer,maliciously changed, or the certificate is wrong
            (can also be all of them at the same time)".to_string(),
        CertificateState::UnknownCertState => "\n\nCertificate not known. NOTE: contribution welcomed".to_string()
    }
}

pub fn get_certificate(package_name: &str, user_id: Option<u16>, device_serial: &str) -> String {
    let cert_name = "CERT.RSA";
    let package_path = AdbCommand::new()
        .shell(device_serial)
        .pm()
        .grab_package_path(package_name, user_id)
        .unwrap_or_default();

    //TODO not sure where to put the temporary files
    let filename = package_path.split('/').last().unwrap_or(&package_path);
    AdbCommand::new().pull_package(&package_path).expect("failed to pull package from system");
    unzip_package(filename, cert_name).expect("failed to unzip/delete package");
    extract_certificate(&cert_name)
}

pub fn unzip_package(package_name: &str, cert_name: &str) -> io::Result<()> {
    {
        let file = File::open(package_name)?;
        let reader = BufReader::new(file);
        let mut archive = ZipArchive::new(reader)?;
    
        let mut zip_file = archive.by_name("META-INF/CERT.RSA")?;
    
        let mut output = File::create(cert_name)?;
        io::copy(&mut zip_file, &mut output)?;
        fs::remove_file(package_name)?; 
    }
    Ok(())
}

pub fn extract_certificate(cert_name: &str) -> String {
    let output = Command::new("openssl")
        .args(&["pkcs7", "-in", cert_name, "-inform", "DER", "-print_certs", "-noout"])
        .output()
        .expect("Failed to execute openssl command");

    fs::remove_file(cert_name).expect("failed to remove certificate file");
    String::from_utf8_lossy(&output.stdout).trim().replace('\n', " ").to_string()
}
