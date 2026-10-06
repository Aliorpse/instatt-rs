# Drepreacted

# instatt-rs

Unofficial Rust client for the UNM Instatt attendance backend. Supports student accounts only.

## Usage

```toml
[dependencies]
instatt-rs = { version = "..." }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The example below will generate an Azure sign-in URL, you need to complete sign-in in your browser, then paste the full callback URL. The browser will fail to load `https://localhost`; copy the URL from its address bar anyway.

```rust,no_run
use instatt::{InstattClient, authorize_url, upn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Open: {}", authorize_url(&upn("your-username")));
    println!("Paste the full callback URL:");

    let mut callback = String::new();
    std::io::stdin().read_line(&mut callback)?;
    let client = InstattClient::login_with_auth_url(callback.trim()).await?;

    for class in client.today().await? {
        println!("{} {}–{} @ {}", class.module.id, class.start, class.end, class.venue);
    }

    Ok(())
}
```

## Things to know

- Authorization codes are short-lived and single-use. To resume later, store the value returned by `firebase_refresh_token()` and use `restore_with_firebase_refresh_token()`.

## Disclaimer

This library is not affiliated with UNM. It was bulit to address shortcomings in the official app, such as the lack of attendance grouping. The author assumes no liability for any unlawful use of this library or any violation of UNM regulations. Use at your own risk.
