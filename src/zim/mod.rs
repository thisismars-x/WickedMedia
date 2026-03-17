/*
 * This file defines a complete zim file disassembler
 * Read at `https://wiki.openzim.org/wiki/ZIM_file_format`
 */

#![allow(unused_variables, dead_code)]

use std::io::{Read, Write, Seek, SeekFrom, Result, Error, ErrorKind};
use std::fs::{File, self};
use std::path::{Path};
use std::collections::HashMap;
use xz2::read::XzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

fn read_1_bytes(file: &mut File) -> Result<u8>{
    let mut buffer = [0u8; 1];
    file.read_exact(&mut buffer)?;

    Ok(u8::from_le_bytes(buffer))
}
fn read_2_bytes(file: &mut File) -> Result<u16>{
    let mut buffer = [0u8; 2];
    file.read_exact(&mut buffer)?;

    Ok(u16::from_le_bytes(buffer))
}

fn read_4_bytes(file: &mut File) -> Result<u32>{
    let mut buffer = [0u8; 4];
    file.read_exact(&mut buffer)?;

    Ok(u32::from_le_bytes(buffer))
}

fn read_8_bytes(file: &mut File) -> Result<u64>{
    let mut buffer = [0u8; 8];
    file.read_exact(&mut buffer)?;

    Ok(u64::from_le_bytes(buffer))
}

fn read_16_bytes(file: &mut File) -> Result<u128>{
    let mut buffer = [0u8; 16];
    file.read_exact(&mut buffer)?;

    Ok(u128::from_le_bytes(buffer))
}

fn read_string_upto_null_byte(file: &mut File) -> Result<String> {
    let mut vec: Vec<u8> = Vec::new();
    let mut buffer = [0u8];

    loop {
        file.read_exact(&mut buffer)?;
        if buffer[0] == 0 { break; }
        vec.push(buffer[0]);
    }

    Ok(String::from_utf8_lossy(&vec).into())
}

/// Magic number used to recognize zim file formats
pub static MAGIC_HEADER: u32 = 0x44D495A;

/// 80 byte preamble in zim file format
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZimHeader {

    /// Recorded, although never used after assertion check to see if this header is valid. 
    magic_number: u32,

    /// Major version of this header
    pub maj_version: u16,
    /// Minor version of this header
    pub min_version: u16,

    /// Unique id of this zim archive
    pub uuid: u128,

    /// Total number of directory entries
    pub entry_count: u32,

    /// Total number of clusters
    pub cluster_count: u32,

    /// Position of directory pointer list, ordered by Path
    pub path_ptr_pos: u64,

    /// OBSOLETE! Position of directory pointer list, ordered by Title
    pub title_ptr_pos: u64,

    /// Position of cluster pointer list
    pub cluster_ptr_pos: u64,

    /// Position of mime-type list
    pub mime_list_pos: u64,

    /// 0xffffffff if no main page
    pub main_page: u32,

    /// DEPRECATED! Always 0xffffffff
    pub layout_page: u32,

    /// pointer to md5checksum without the checksum itself
    pub checksum_pos: u64,

}

impl ZimHeader {

    /// Initialize a new ZimHeader
    /// Be careful to start at byte offset 0, or else will throw an error
    pub fn new(file: &mut File) -> Result<Self> {
        file.seek(SeekFrom::Start(0))?;

        Ok(
            Self {
                magic_number: {
                    let magic_number = read_4_bytes(file)?;
                    assert_eq!(magic_number, MAGIC_HEADER, "Unrecognized file format. Conflicting magic-number!");
                    magic_number
                },
                maj_version: read_2_bytes(file)?,
                min_version: read_2_bytes(file)?,
                uuid: read_16_bytes(file)?,
                entry_count: read_4_bytes(file)?,
                cluster_count: read_4_bytes(file)?,
                path_ptr_pos: read_8_bytes(file)?,
                title_ptr_pos: read_8_bytes(file)?,
                cluster_ptr_pos: read_8_bytes(file)?,
                mime_list_pos: read_8_bytes(file)?,
                main_page: read_4_bytes(file)?,
                layout_page: read_4_bytes(file)?,
                checksum_pos: read_8_bytes(file)?,
            }
        )
    }
}

#[derive(Clone, Debug)]
pub struct ZimMimeTypeList {

    /// Mime types are stored by index and mime-type. Each article in 
    /// this zim file references mime_types to find out what type of mime is being used
    pub mime_types: HashMap<u16, String>,
}

impl ZimMimeTypeList {

    /// Initialize MimeList after the ZimHeader, starting at offset mime_list_pos
    pub fn new(file: &mut File, mime_list_pos: u64) -> Result<Self> {
        file.seek(SeekFrom::Start(mime_list_pos))?;

        let mut mime_type;
        let mut mime_types = HashMap::new();

        
        for i in 0.. {
            mime_type = read_string_upto_null_byte(file)?;
            if mime_type.is_empty() { break; }
            mime_types.insert(i, mime_type);
        }

        Ok(Self{mime_types})
    }

    pub fn which_mime(&self, mime_type: u16) -> &str {
        let default_mime_type = "dat".to_string();
        let mime_type = self.mime_types.get(&mime_type).unwrap_or(&default_mime_type).as_str();
        match mime_type {

            // Core Web content
            "text/html" => "html",
            "text/css" => "css",
            "text/javascript" | "application/javascript" => "js",
            "text/plain" => "txt",

            // Images
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/jpg" => "jpg",
            "image/gif" => "gif",
            "image/svg+xml" => "svg",
            "image/webp" => "webp",
            "image/bmp" => "bmp",
            "image/x-icon" | "image/vnd.microsoft.icon" => "ico",

            // Documents or other content
            "application/pdf" => "pdf",
            "application/zip" => "zip",
            "application/octet-stream" => "bin",

            // Audio or video
            "audio/mpeg" => "mp3",
            "audio/ogg" => "ogg",
            "audio/wav" => "wav",
            "video/mp4" => "mp4",
            "video/ogg" => "ogv",
            "video/webm" => "webm",

            _ => "dat",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ZimPathPointerList {
    
    /// Ordered by path(<namespace><path>), using vector to retain order
    /// Also, importantly, this should hold, path_pointer_list.len() == header.entry_count
    pub path_pointer_list: Vec<u64>,
}

impl ZimPathPointerList {
    
    /// Extracts `entry_count` number of path pointers from the zim file
    /// entry_count should be from ZimHeader
    pub fn new(file: &mut File, path_ptr_pos: u64, entry_count: u32) -> Result<Self> {
        file.seek(SeekFrom::Start(path_ptr_pos))?;
        
        let mut path_pointer_list = Vec::new();
        for _ in 0..entry_count {
            path_pointer_list.push(read_8_bytes(file)?);
        }

        Ok(Self{ path_pointer_list })
    }
}

#[derive(Clone, Debug)]
pub enum ZimDirectoryEntry {

    Content {
        /// MIME type defined in ZimMimeTypeList
        mime_type: u16,

        /// NOT USED!
        param_len: u8,

        /// which namespace does this entry belong to
        name_space: char,

        /// NOT USED! to know if this entry has been updated
        revision: u32,

        /// which cluster number does this entry belong to 
        cluster_number: u32,

        /// inside the compressed cluster, which blob number does this 
        /// entry belong to 
        blob_number: u32,

        /// path as referred to in the path pointer list
        path: String,

        /// title as referred to in title pointer list, same as path usually
        title: String,

        /// NOT USED!
        parameter: u16,
    },

    Redirect {
        /// MIME type defined in ZimMimeTypeList
        mime_type: u16,

        /// NOT USED!
        param_len: u8,

        /// which namespace does this entry belong to
        name_space: char,

        /// NOT USED! to know if this entry has been updated
        revision: u32,

        /// pointer to the directory entry of redirect target
        redirect_index: u32,

        /// path as referred to in the path pointer list
        path: String,

        /// title as referred to in title pointer list, same as path usually
        title: String,

        /// NOT USED!
        parameter: u16,
    }

}

/// List of ZimDirectoryEntry(ies) pointed to by path_ptr_pos
#[derive(Clone, Debug)]
pub struct ZimDirectoryEntryList(pub Vec<ZimDirectoryEntry>);

/// Path pointer points to directory entries with first byte mime_type 
/// dictating if this entry is one of two ZimDirectoryEntryTypes(Content or Redirect)
static REDIRECT_ENTRY: u16 = 0xffff;

impl ZimDirectoryEntryList {

    /// Initialize ZimDirectoryEntryList with directory entries pointed
    /// to by path_ptr_list from ZimPathPointerList
    pub fn new(file: &mut File, path_ptr_list: Vec<u64>) -> Result<Self> {
        let mut dir_entry_list = Vec::new(); 

        for path_ptr in path_ptr_list {
            file.seek(SeekFrom::Start(path_ptr))?;

            let mime_type = read_2_bytes(file)?;
            let param_len = read_1_bytes(file)?;
            let name_space: char = read_1_bytes(file)?.into();
            let revision = read_4_bytes(file)?;

            if mime_type == REDIRECT_ENTRY {
                let redirect_index = read_4_bytes(file)?;
                let path = read_string_upto_null_byte(file)?;
                let title = read_string_upto_null_byte(file)?;
                let parameter = read_2_bytes(file)?;

                dir_entry_list.push(
                    ZimDirectoryEntry::Redirect {
                        mime_type, 
                        param_len,
                        name_space, 
                        revision,
                        redirect_index,
                        path,
                        title,
                        parameter,
                    }
                )
            } else {
                let cluster_number = read_4_bytes(file)?;
                let blob_number = read_4_bytes(file)?;
                let path = read_string_upto_null_byte(file)?;
                let title = {
                    let title = read_string_upto_null_byte(file)?;
                    if title.is_empty() { path.clone() }
                    else { title }
                };
                let parameter = read_2_bytes(file)?;

                dir_entry_list.push(
                    ZimDirectoryEntry::Content {
                        mime_type,
                        param_len,
                        name_space,
                        revision,
                        cluster_number,
                        blob_number,
                        path,
                        title,
                        parameter,
                    }
                )
            }

        }

        Ok(ZimDirectoryEntryList(dir_entry_list))
    }
}

pub struct ZimClusterPointerList {

    /// List of all pointers later used by ZimCluster
    pub cluster_ptr_list: Vec<u64>,
}

impl ZimClusterPointerList {
    pub fn new(file: &mut File, cluster_ptr_pos: u64, cluster_count: u32) -> Result<Self> {
        file.seek(SeekFrom::Start(cluster_ptr_pos))?;

        let mut cluster_ptr_list = Vec::new();
        for _ in 0..cluster_count {
            cluster_ptr_list.push(read_8_bytes(file)?); 
        }

        Ok(Self{ cluster_ptr_list })
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum CompressionType {

    /// Fourth low bits of cluster = 1
    NONE = 1, 

    /// Fourth low bits of cluster = 4 
    LZMA2 = 4,

    /// Fourth low bits of cluster = 5 
    ZSTD = 5,

}

pub struct ZimCluster {

    /// Each blob stored as a vector of bytes
    pub blobs: Vec<Vec<u8>>,

    /// compression types are stored in the first low four bits
    /// Fourth low bits : 1: no compression, 4: LZMA2 compressed, 5: zstd compressed
    /// Fifth bits : 0: normal (OFFSET_SIZE=4) 1: extended (OFFSET_SIZE=8)
    pub compression_type: CompressionType,

    /// offset_size determines if the cluster is extended or not 
    /// clusters before v6 had no option to be extended
    pub offset_size: u8,
}

impl ZimCluster {

    pub fn new(file: &mut File, cluster_number: u32, cluster_ptr_list: Vec<u64>) -> Result<Self> {

        let start = cluster_ptr_list[cluster_number as usize];
        let end= if (cluster_number as usize + 1) < cluster_ptr_list.len() {
            cluster_ptr_list[cluster_number as usize + 1]
        } else {
            file.metadata()?.len() // last cluster goes to end of file
        };
        file.seek(SeekFrom::Start(start))?;

        let cluster_size = (end-start) as usize;
        let mut raw_data = vec![0u8; cluster_size];
        file.read_exact(&mut raw_data)?;

        let (compression_type, uncompressed_data) = {
            let com_type = raw_data[0] & 0xf;
            match com_type {
                1 => {
                    (CompressionType::NONE, raw_data[1..].to_vec())
                },

                4 => {
                    let mut decoder = XzDecoder::new(&raw_data[1..]);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    (CompressionType::LZMA2, buf)
                },

                5 => {
                    let mut decoder = ZstdDecoder::new(&raw_data[1..])?;
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf)?;
                    (CompressionType::ZSTD, buf)
                },

                _ => return Err(Error::new(ErrorKind::Other, "Invalid `compression_type` in cluster info")),
            }
        };

        let offset_size = {
           let extended = ((raw_data[0] >> 4) & 0x01) == 1; 
           if extended { 8 } else { 4 }
        };

        // reading offsets
        let mut offsets = Vec::new();

        let first_offset = if offset_size == 4 {
            u32::from_le_bytes(uncompressed_data[0..4].try_into().unwrap()) as usize
        } else {
            u64::from_le_bytes(uncompressed_data[0..8].try_into().unwrap()) as usize
        };

        let num_offsets = first_offset / offset_size;
        for i in 0..num_offsets {
            let pos = i * offset_size;

            let offset = if offset_size == 4 {
                u32::from_le_bytes(uncompressed_data[pos..pos + 4].try_into().unwrap()) as usize
            } else {
                u64::from_le_bytes(uncompressed_data[pos..pos + 8].try_into().unwrap()) as usize
            };

            offsets.push(offset);
        }

        // reading blobs
        let mut blobs = Vec::new();
        for i in 0..offsets.len() - 1 {
            let (blob_start, blob_end) = (offsets[i], offsets[i+1]);
            if blob_end > uncompressed_data.len() || blob_start > blob_end { 
                break;
            }

            blobs.push(uncompressed_data[blob_start..blob_end].to_vec());
        }

        Ok(Self { blobs, compression_type, offset_size: offset_size as u8 })
    }

    pub fn get_blob_at(&self, index: u32) -> &Vec<u8> {
        &self.blobs[index as usize]
    }

}

pub struct ZimClient<P: AsRef<Path>> {

    /// Where are the .zim files stored?
    store: P,

    /// Where do you want to store the extracted files?
    /// output has two subfolders _res_ and _mw_
    /// _res_ is for wikipedia styles that apply across pages, footer.css, vector.css, favicon.png
    /// _mw_ is for assets under each page
    output: P,
}

impl<P: AsRef<Path> + Sync> ZimClient<P> {

    /// Freshly initialized client, reuse for multiple queries
    pub fn new(store: P, output: P) -> Result<Self> {
        let (store_path, output_path) = (store.as_ref(), output.as_ref());

        // if no store directory exists, download some zim files into store directory
        if !store_path.exists() || !store_path.is_dir() {
            return Err(Error::new(ErrorKind::NotFound, "`store` either does not exist(no .zim files, try downloading) or is not a directory"));
        }
        if !output_path.exists() {
            fs::create_dir_all(&output)?;
        }

        Ok(Self {
            store, 
            output, 
        })
    }

    /// Extract all files, and place them under C/ M/ X/ namespaces in OUTPUT_DIR
    /// Note that if some file fails during extraction, silently skips and continues 
    /// with other clusters. There are millions of files, so.
    ///
    /// This should not take long, parallelized with `rayon`!
    pub fn extract_zim_file(&self, file: &mut File) -> Result<()> {
        let zim_header = ZimHeader::new(file)?;
        let zim_mime_types = ZimMimeTypeList::new(file, zim_header.mime_list_pos)?;
        let zim_ptr_list = ZimPathPointerList::new(file, zim_header.path_ptr_pos, zim_header.entry_count)?;
        let zim_dir_entry_list = ZimDirectoryEntryList::new(file, zim_ptr_list.path_pointer_list)?;
        let zim_cluster_list = ZimClusterPointerList::new(file, zim_header.cluster_ptr_pos, zim_header.cluster_count)?;

        let (entries, file) = (Arc::new(zim_dir_entry_list.0), Arc::new(Mutex::new(file)));
        let cluster_cache: Arc<Mutex<HashMap<u32, ZimCluster>>> = Arc::new(Mutex::new(HashMap::new()));

        entries.par_iter().for_each(|entry| {
            let mut current = entry;

            loop {
                match current {

                    ZimDirectoryEntry::Content {
                        mime_type,
                        name_space,
                        cluster_number,
                        blob_number,
                        path,
                        ..
                    } => {

                        let mut cache = cluster_cache.lock().unwrap();

                        if !cache.contains_key(cluster_number) {

                            let mut file_guard = file.lock().unwrap();

                            let cluster = match ZimCluster::new(
                                &mut *file_guard,
                                *cluster_number,
                                zim_cluster_list.cluster_ptr_list.clone(),
                            ) {
                                Ok(c) => c,
                                Err(_) => return,
                            };

                            cache.insert(*cluster_number, cluster);
                        }

                        let data_blob = {
                            let zim_cluster = cache.get(cluster_number).unwrap();
                            zim_cluster.get_blob_at(*blob_number).to_vec()
                        };

                        drop(cache);

                        println!("Extracting {}:{}", name_space, path);

                        let ext = zim_mime_types.which_mime(*mime_type);

                        let mut output_path = self
                            .output
                            .as_ref()
                            .join(name_space.to_string())
                            .join(path);

                        if output_path.extension().is_none() {
                            output_path.set_extension(ext);
                        }

                        if let Some(parent) = output_path.parent() {
                            if fs::create_dir_all(parent).is_err() {
                                return;
                            }
                        }

                        if let Ok(mut out_file) = File::create(output_path) {
                            let _ = out_file.write_all(&data_blob);
                        }

                        break;
                    }

                    ZimDirectoryEntry::Redirect {
                        redirect_index,
                        ..
                    } => {
                        current = &entries[*redirect_index as usize];
                    }
                }
            }

        });

        Ok(())
    }
}

