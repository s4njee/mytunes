use mt_app::{ArtworkSize, library::DesktopLibrary};
use tauri::http::{Method, Request, Response, StatusCode, header};

struct ArtworkRequest<'a> {
    id: &'a str,
    size: ArtworkSize,
}

fn parse(request: &Request<Vec<u8>>) -> Result<ArtworkRequest<'_>, StatusCode> {
    if request.method() != Method::GET {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    let (id, size) = request
        .uri()
        .path()
        .strip_prefix('/')
        .and_then(|path| path.split_once('/'))
        .ok_or(StatusCode::NOT_FOUND)?;
    if id.len() != 64 || !id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(StatusCode::NOT_FOUND);
    }
    let size = match size {
        "300" => ArtworkSize::Small,
        "600" => ArtworkSize::Large,
        _ => return Err(StatusCode::NOT_FOUND),
    };
    Ok(ArtworkRequest { id, size })
}

pub(crate) fn serve(
    library: Option<&DesktopLibrary>,
    request: &Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let result = parse(request).and_then(|art| {
        library
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
            .artwork(art.id, art.size)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)
    });
    match result {
        Ok(bytes) => {
            let mut response = Response::new(bytes);
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("image/jpeg"),
            );
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            response
        }
        Err(status) => {
            let mut response = Response::new(Vec::new());
            *response.status_mut() = status;
            response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artwork_protocol_reads_only_known_digests_and_sizes() {
        let dir = tempfile::tempdir().expect("create temporary directory");
        let path = dir.path().join("library.sqlite");
        let library = DesktopLibrary::open(&path).expect("open a test library");
        let db = mt_db::open_desktop(path).expect("open test library for seeding");
        let id = "ab".repeat(32);
        db.execute(
            "INSERT INTO artwork (id, thumb_300, thumb_600) VALUES (?1, X'1234', X'5678')",
            [&id],
        )
        .expect("insert sample artwork");
        for (uri, bytes) in [
            (format!("mtart://localhost/{id}/300"), [0x12, 0x34]),
            (format!("http://mtart.localhost/{id}/600"), [0x56, 0x78]),
        ] {
            let request = Request::get(uri)
                .body(Vec::new())
                .expect("construct request");
            let response = serve(Some(&library), &request);
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "public, max-age=31536000, immutable"
            );
            assert_eq!(response.body(), &bytes);
        }
        for uri in [
            format!("mtart://localhost/{id}/999"),
            "mtart://localhost/%2e%2e/300".into(),
            "mtart://localhost/file/path/300".into(),
            format!("mtart://localhost/{}/300", "0".repeat(64)),
        ] {
            let request = Request::get(uri)
                .body(Vec::new())
                .expect("construct request");
            assert_eq!(
                serve(Some(&library), &request).status(),
                StatusCode::NOT_FOUND
            );
        }
    }
}
