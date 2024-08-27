use rouille::{Request, Response};

pub fn start_http_thread(port: u16, report_data: [u8; 64]) {
    let (quote, collateral) = crate::attest::generate_for_report_data(report_data)
        .expect("failed to generate http report data");

    std::thread::spawn(move || {
        let router = move |request: &Request| {
            rouille::router!(request,
                (GET)(/quote) => {
                    Response::from_data("raw", quote.clone())
                },
                (GET)(/collateral) => {
                    Response::from_data("application/json", serde_json::to_string(&collateral).unwrap())
                },
                _ => {
                    Response::empty_404()
                }
            )
        };

        rouille::start_server(("0.0.0.0", port), router);
    });
}
