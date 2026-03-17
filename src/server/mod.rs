use actix_files::NamedFile;
use actix_web::{get, post, web, HttpResponse, Responder, HttpServer, App};
use serde::{Deserialize, Serialize};
use std::fs;
use std::env::var;
use std::path::PathBuf;

/// Port number used by default, if busy, come here and change it
const DEFAULT_PORT_NO: u16 = 8999;

#[derive(Deserialize)]
struct SearchQuery {
    query: String,
}

#[derive(Serialize)]
struct SearchResult {
    title: String,
    path: String,
}

const ARTICLES_DIR: &str = "./output/C";
const BASE_DIR: &str = "./base";

#[get("/")]
pub async fn index() -> impl Responder {
    let base_dir = var("JENNIFER").expect("SET `$JENNIFER` variable to know base path of assets for the life of this project");
    NamedFile::open(format!("{}/jennifer/{}/index.html", base_dir, BASE_DIR))
}

#[post("/search")]
pub async fn search_articles(data: web::Json<SearchQuery>) -> impl Responder {
    let query = data.query.to_lowercase();
    let mut results = Vec::new();

    let base_dir = var("JENNIFER").expect("SET `$JENNIFER` variable to know base path of assets for the life of this project");
    let articles_dir = format!("{}/jennifer/{}", base_dir, ARTICLES_DIR);
    if let Ok(entries) = fs::read_dir(articles_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == "html" {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        let title = filename.replace("_", " ");
                        if title.to_lowercase().contains(&query) {
                            results.push(SearchResult {
                                title: title.clone(),
                                path: filename.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    HttpResponse::Ok().json(results)
}

#[get("/article/{page:.*}")]
pub async fn show_article(page: web::Path<String>) -> impl actix_web::Responder {
    let base_dir = var("JENNIFER").expect("SET `$JENNIFER` variable to know base path of assets for the life of this project");
    let mut filepath = PathBuf::from(format!("{}/jennifer/output", base_dir));
    let page = page.into_inner(); // extract the String from Path
    filepath.push(&page);

    if !filepath.exists() && !page.ends_with(".html") {
        filepath.set_extension("html");
    }

    if filepath.exists() {
        actix_files::NamedFile::open(filepath)
    } else {
        actix_files::NamedFile::open(format!("{}/404.html", BASE_DIR))
    }
}


/// Main app instance, nothing special
#[actix_web::main]
pub async fn app() -> std::io::Result<()> {
    println!("LOG: jennifer is online at localhost:{DEFAULT_PORT_NO}");
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(search_articles)
            .service(show_article)
    })
    .bind(("127.0.0.1", DEFAULT_PORT_NO))?
    .run()
    .await
}

