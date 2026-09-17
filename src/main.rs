use axum::{
    extract::Query,
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use image::{imageops, GenericImageView, ImageFormat, Rgba};
use qrcode::{EcLevel, QrCode};
use serde::Deserialize;
use std::io::Cursor;

#[derive(Deserialize)]
struct QrParams {
    url: String,
}

// 1. The new handler to serve the HTML frontend
async fn serve_form() -> Html<&'static str> {
    Html(r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>QR Code Generator</title>
            <style>
                body { font-family: system-ui, -apple-system, sans-serif; display: flex; flex-direction: column; align-items: center; margin-top: 50px; background: #f4f4f5; }
                .container { background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); text-align: center; }
                input { padding: 10px; width: 300px; margin-bottom: 10px; border: 1px solid #ccc; border-radius: 6px; font-size: 16px; }
                button { padding: 10px 20px; background: #2563eb; color: white; border: none; border-radius: 6px; font-size: 16px; cursor: pointer; }
                button:hover { background: #1d4ed8; }
                #qr-container { margin-top: 20px; min-height: 250px; display: flex; justify-content: center; align-items: center; }
                img { max-width: 250px; border-radius: 8px; display: none; }
            </style>
        </head>
        <body>
            <div class="container">
                <h2>Generate Branded QR Code</h2>
                <form id="qr-form">
                    <input type="url" id="url-input" placeholder="Enter your link (e.g., https://example.com)" required>
                    <br>
                    <button type="submit">Generate</button>
                </form>
                <div id="qr-container">
                    <img id="qr-result" alt="Generated QR Code" />
                </div>
            </div>

            <script>
                document.getElementById('qr-form').addEventListener('submit', function(e) {
                    e.preventDefault();
                    const url = document.getElementById('url-input').value;
                    const img = document.getElementById('qr-result');
                    
                    // Point the image source directly to our Axum endpoint
                    img.src = `/qr?url=${encodeURIComponent(url)}`;
                    img.style.display = 'block';
                });
            </script>
        </body>
        </html>
    "#)
}

async fn generate_qr(Query(params): Query<QrParams>) -> impl IntoResponse {
    let code = match QrCode::with_error_correction_level(params.url.as_bytes(), EcLevel::H) {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode QR payload").into_response(),
    };

    let mut qr_img = code.render::<Rgba<u8>>()
        .min_dimensions(400, 400)
        .dark_color(Rgba([0, 0, 0, 255]))
        .light_color(Rgba([255, 255, 255, 255]))
        .build();

    let logo_bytes = include_bytes!("logo.png");
    let logo = image::load_from_memory(logo_bytes).expect("Failed to decode embedded logo");

    let (qr_width, qr_height) = qr_img.dimensions();
    let max_logo_size = qr_width / 4; 
    let resized_logo = logo.resize(max_logo_size, max_logo_size, imageops::FilterType::Lanczos3);
    
    let (logo_width, logo_height) = resized_logo.dimensions();
    let x_offset = (qr_width - logo_width) / 2;
    let y_offset = (qr_height - logo_height) / 2;

    imageops::overlay(&mut qr_img, &resized_logo, x_offset as i64, y_offset as i64);

    let mut buffer = Cursor::new(Vec::new());
    if qr_img.write_to(&mut buffer, ImageFormat::Png).is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to generate PNG bytes").into_response();
    }

    (
        [(header::CONTENT_TYPE, "image/png")],
        buffer.into_inner(),
    ).into_response()
}

#[tokio::main]
async fn main() {
    // 2. Add the root route mapping to our HTML handler
    let app = Router::new()
        .route("/", get(serve_form))
        .route("/qr", get(generate_qr));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    
    axum::serve(listener, app).await.unwrap();
}