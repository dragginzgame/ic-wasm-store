use ic_testkit::pocket_ic::{PocketIcBuilder, nonblocking::PocketIc};

pub fn wasm() -> Vec<u8> {
    std::fs::read(
        std::env::var_os("IC_WASM_STORE_WASM")
            .expect("run make qualification-test to select the Cargo-built Wasm"),
    )
    .expect("read probe Wasm")
}

pub async fn instance() -> PocketIc {
    let url = std::env::var("IC_TESTKIT_POCKET_IC_URL")
        .expect("run make qualification-test or supply an externally owned server URL");
    PocketIcBuilder::new()
        .with_server_url(url.parse().expect("valid PocketIC server URL"))
        .with_max_request_time_ms(Some(30_000))
        .with_application_subnet()
        .build_async()
        .await
}
