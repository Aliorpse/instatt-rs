use std::error::Error;
use std::io::{self, Write};

use instatt::{InstattClient, authorize_url, upn};

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn Error>> {
    let client = match std::env::var("INSTATT_REFRESH_TOKEN") {
        Ok(refresh_token) if !refresh_token.trim().is_empty() => {
            println!("Restoring the Firebase session from INSTATT_REFRESH_TOKEN...");
            InstattClient::restore_with_firebase_refresh_token(refresh_token.trim()).await?
        }
        _ => login().await?,
    };

    let refresh_token = client
        .firebase_refresh_token()
        .await
        .ok_or("the session did not contain a Firebase refresh token")?;

    println!();
    println!("Student: {}", client.student_id());
    println!("Firebase refresh token:");
    println!("{refresh_token}");
    println!();
    println!("Store this token securely. Do not commit it or share it.");

    Ok(())
}

async fn login() -> Result<InstattClient, Box<dyn Error>> {
    let username = prompt("Username or UPN: ")?;
    let login_hint = upn(&username);

    println!();
    println!("Open this URL in a browser and complete the Azure sign-in:");
    println!("{}", authorize_url(&login_hint));
    println!();

    let callback_url = prompt("Paste the callback URL: ")?;
    println!("Signing in...");

    Ok(InstattClient::login_with_auth_url(&callback_url).await?)
}

fn prompt(message: &str) -> io::Result<String> {
    print!("{message}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_owned())
}
