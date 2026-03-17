
#![allow(dead_code)]

use jennifer::zim::*;
use std::fs::File;
use std::io::{Read};

static TEST_FILE: &str = "./store/wikipedia_en_physics_maxi_2026-01.zim";

#[test]
fn invalid_file_offset() {
    let mut file = File::open(TEST_FILE).unwrap();
    let mut byte_1_buffer = [0u8];
    file.read_exact(&mut byte_1_buffer).unwrap();

    // Although moved by 1 byte, resets by call to SeekFrom::Start(0) in new
    let _zim_header = ZimHeader::new(&mut file).unwrap();
}

#[test]
#[should_panic]
fn invalid_magic_number() {
    let mut file = File::open("src/main.rs").unwrap();
    let _zim_header = ZimHeader::new(&mut file).expect("Opened a regular file for reading");
}

#[test]
fn collect_mime_type_list() {
    let mut file = File::open(TEST_FILE).unwrap();
    let zim_header = ZimHeader::new(&mut file).unwrap();
    let _zim_mime_list = ZimMimeTypeList::new(&mut file, zim_header.mime_list_pos).expect("The file index has moved. Should not fail");
}

#[test]
fn collect_path_pointers() {
    let mut file = File::open(TEST_FILE).unwrap();
    let zim_header = ZimHeader::new(&mut file).unwrap();
    let _zim_mime_list = ZimMimeTypeList::new(&mut file, zim_header.mime_list_pos).unwrap();

    let _zim_path_pointers = ZimPathPointerList::new(&mut file, zim_header.path_ptr_pos, zim_header.entry_count).expect("Since mimetype lists have moved the file. Should not fail");
}

#[test] // run with --no-capture
fn collect_path_entries() {
    let mut file = File::open(TEST_FILE).unwrap();
    let zim_header = ZimHeader::new(&mut file).unwrap();
    let zim_path_pointer_list = ZimPathPointerList::new(&mut file, zim_header.path_ptr_pos, zim_header.entry_count).unwrap();
    let zim_dir_entry_list = ZimDirectoryEntryList::new(&mut file, zim_path_pointer_list.path_pointer_list.clone()).unwrap();

    let mut n_redirect: u32 = 0;
    let mut n_content: u32 = 0;
    for entries in zim_dir_entry_list.0 {
        match entries {
            ZimDirectoryEntry::Redirect{path, title, redirect_index, ..} => {
                println!("REDIRECT: {path} | {title} | 0x{:x}", redirect_index);
                n_redirect += 1;
            },
            ZimDirectoryEntry::Content{path, title, ..} => {
                println!("CONTENT: {path} | {title}");
                n_content += 1;
            }
        }
    }

    println!("Number of redirects: {}, number of contents: {}", n_redirect, n_content);
    assert_eq!(n_redirect + n_content, zim_path_pointer_list.path_pointer_list.len() as u32);
}

#[test]
fn collect_cluster_ptr_list() {
    let mut file = File::open(TEST_FILE).unwrap();
    let zim_header = ZimHeader::new(&mut file).unwrap();
    let zim_cluster_ptr_list = ZimClusterPointerList::new(&mut file, zim_header.cluster_ptr_pos, zim_header.cluster_count).unwrap();
    for ptr in zim_cluster_ptr_list.cluster_ptr_list {
        println!("0x{:x}", ptr);
    }
}


