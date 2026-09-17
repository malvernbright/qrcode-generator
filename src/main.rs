use axum::{
    extract::Multipart,
    http::{header, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use image::{imageops, GenericImageView, ImageFormat, Rgba};
use qrcode::{EcLevel, QrCode};
use std::io::Cursor;

async fn serve_form() -> Html<&'static str> {
    Html(r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>Branded QR Code Generator</title>
            <style>
                body { font-family: system-ui, -apple-system, sans-serif; display: flex; flex-direction: column; align-items: center; margin-top: 50px; background: #f4f4f5; }
                .container { background: white; padding: 30px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); text-align: center; width: 360px; }
                .field { margin-bottom: 15px; text-align: left; }
                label { display: block; font-size: 14px; font-weight: 600; margin-bottom: 5px; color: #374151; }
                input[type="url"], input[type="file"] { width: 100%; box-sizing: border-box; padding: 10px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; }
                button { width: 100%; padding: 12px; background: #2563eb; color: white; border: none; border-radius: 6px; font-size: 16px; font-weight: 600; cursor: pointer; margin-top: 10px; }
                button:hover { background: #1d4ed8; }
                #qr-container { margin-top: 25px; min-height: 250px; display: flex; justify-content: center; align-items: center; }
                img { max-width: 250px; border-radius: 8px; display: none; }
            </style>
        </head>
        <body>
            <div class="container">
                <h2>Branded QR Generator</h2>
                <form id="qr-form">
                    <div class="field">
                        <label for="url-input">Target URL</label>
                        <input type="url" id="url-input" placeholder="https://example.com" required>
                    </div>
                    <div class="field">
                        <label for="logo-input">Custom Logo (Optional)</label>
                        <input type="file" id="logo-input" accept="image/png, image/jpeg">
                    </div>
                    <button type="submit">Generate QR Code</button>
                </form>
                <div id="qr-container">
                    <img id="qr-result" alt="Generated QR Code" />
                </div>
            </div>

            <script>
                document.getElementById('qr-form').addEventListener('submit', async function(e) {
                    e.preventDefault();
                    const url = document.getElementById('url-input').value;
                    const logoInput = document.getElementById('logo-input');
                    const img = document.getElementById('qr-result');

                    const formData = new FormData();
                    formData.append('url', url);
                    if (logoInput.files.length > 0) {
                        formData.append('logo', logoInput.files[0]);
                    }

                    try {
                        const response = await fetch('/qr', {
                            method: 'POST',
                            body: formData
                        });

                        if (!response.ok) {
                            alert('Failed to generate QR code.');
                            return;
                        }

                        const blob = await response.blob();
                        img.src = URL.createObjectURL(blob);
                        img.style.display = 'block';
                    } catch (err) {
                        console.error(err);
                        alert('An error occurred while generating the QR code.');
                    }
                });
            </script>
        </body>
        </html>
    "#)
}

async fn generate_qr(mut multipart: Multipart) -> impl IntoResponse {
    let mut url_opt: Option<String> = None;
    let mut uploaded_logo_bytes: Option<Vec<u8>> = None;

    // 1. Extract payload fields from multipart stream
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "url" {
            if let Ok(text) = field.text().await {
                url_opt = Some(text);
            }
        } else if name == "logo" {
            if let Ok(bytes) = field.bytes().await {
                if !bytes.is_empty() {
                    uploaded_logo_bytes = Some(bytes.to_vec());
                }
            }
        }
    }

    let url = match url_opt {
        Some(u) if !u.is_empty() => u,
        _ => return (StatusCode::BAD_REQUEST, "Missing URL parameter").into_response(),
    };

    // 2. Generate base QR code
    let code = match QrCode::with_error_correction_level(url.as_bytes(), EcLevel::H) {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to encode QR payload").into_response(),
    };

    let mut qr_img = code.render::<Rgba<u8>>()
        .min_dimensions(400, 400)
        .dark_color(Rgba([0, 0, 0, 255]))
        .light_color(Rgba([255, 255, 255, 255]))
        .build();

    // 3. Decode custom logo image (or fall back to embedded logo.png)
    let logo_res = if let Some(bytes) = uploaded_logo_bytes {
        image::load_from_memory(&bytes)
    } else {
        let default_bytes = include_bytes!("logo.jpeg");
        image::load_from_memory(default_bytes)
    };

    let logo = match logo_res {
        Ok(img) => img,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid or unsupported logo image format").into_response(),
    };

    // 4. Resize and overlay the chosen logo
    let (qr_width, qr_height) = qr_img.dimensions();
    let max_logo_size = qr_width / 4; 
    let resized_logo = logo.resize(max_logo_size, max_logo_size, imageops::FilterType::Lanczos3);
    
    let (logo_width, logo_height) = resized_logo.dimensions();
    let x_offset = (qr_width - logo_width) / 2;
    let y_offset = (qr_height - logo_height) / 2;

    imageops::overlay(&mut qr_img, &resized_logo, x_offset as i64, y_offset as i64);

    // 5. Write image buffer to PNG
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
    let app = Router::new()
        .route("/", get(serve_form))
        .route("/qr", post(generate_qr));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");
    
    axum::serve(listener, app).await.unwrap();
}