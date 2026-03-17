use std::fs::{File, self};
use std::path::{Path};
use std::io::Result;
use std::env::var;

mod zim;
mod server;
use zim::ZimClient;

// .zim files should be downloaded and moved over here
const STORE_DIR: &str = "store";
// all  generated files go here, DO NOT CREATE this directory 
// on your own, if you add a new .zim file delete this folder 
// and run this program. 
// Generating the files shouldn't take long at all(rayon is used for that reason)
const OUTPUT_DIR: &str = "output";

fn count_files_recursive(path: &Path) -> usize {
    let mut count = 0;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files_recursive(&path);
            }
        }
    }

    count
}

fn main() -> Result<()> {
    let base_dir = var("JENNIFER").expect("SET `$JENNIFER` variable to know base path of assets for the life of this project");
    let store = Path::new(&base_dir).join("jennifer/store");
    let output = Path::new(&base_dir).join("jennifer/output");

    if !store.exists() {
        panic!("FATAL: No `store/` folder found. All .zim files should go here. Go download .zim files of your chosing: https://download.kiwix.org/zim/wikipedia/")
    }

    if !output.exists() {
        println!("LOG: Opening directory: {:?}", output);
        fs::create_dir_all(output.clone())?;

        let zim_client = ZimClient::new(store.clone(), output.clone())?;

        for entry in fs::read_dir(store.clone())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("zim") {
                println!("LOG: Processing {:?}", path);

                // if extracting a file fails, move on 
                // there are perhaps a million files in the .zim file you're using 
                // a single file does not matter much
                match File::open(&path) {
                    Ok(mut file) => {
                        if let Err(e) = zim_client.extract_zim_file(&mut file) {
                            eprintln!("ERR: Failed to extract {:?}: {}", path, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("ERR: Failed to open {:?}: {}", path, e);
                    }
                }
            }
        }

    } else { 
        // if OUTPUT_DIR already exists, does not extract anything, if you add new files to
        // STORE_DIR, delete OUTPUT_DIR
        println!("LOG: {} exists. Serving now!", OUTPUT_DIR);
    }

    println!("LOG: input zim file stored at `{STORE_DIR}` has {} files.", count_files_recursive(store.as_path())); 
    println!("LOG: generated webpages at `{OUTPUT_DIR}/C` has {} files. [RECURSIVE SEARCH]", count_files_recursive(output.join("C").as_path()));

    // Finally run the server
    server::app()?;
    Ok(())
}

