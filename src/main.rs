use chrono::{DateTime, Utc};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use hex;
use sha1::{Digest, Sha1};
use std::borrow::Cow;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;

/*
Tests
The tester will run your program like this:

$ /path/to/your_program.sh clone https://github.com/blah/blah <some_dir>
Your program must create <some_dir> and clone the given repository into it.

To verify your changes, the tester will:

Check the contents of a random file
Read commit object attributes from the .git directory
*/

//[CONTINUATION PROJECT] - IMPLEMENTATING GIT FROM SCRATCH

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct GitTreeEntry {
    pub mode: String,
    pub name: String,
    pub hash: Hash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hash([u8; 20]);

impl Hash {
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() == 20 {
            let mut array = [0u8; 20];
            array.copy_from_slice(bytes);
            Ok(Hash(array))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid hash length",
            ))
        }
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    eprintln!("Logs from your program will appear here!");

    if args.len() < 2 {
        eprintln!("Usage: {} <command> [<args>]", args[0]);
        return Ok(());
    }

    match args[1].as_str() {
        "init" => {
            fs::create_dir(".git")?;
            fs::create_dir(".git/objects")?;
            fs::create_dir(".git/refs")?;
            fs::create_dir(".git/refs/heads")?;
            fs::create_dir(".git/refs/tags")?;
            fs::write(".git/HEAD", "ref: refs/heads/master\n")?;
            println!("Initialized empty Git repository in .git/");
        }
        "cat-file" => {
            // read the blob object
            // HOW TO IDENTIFY A BLOB FILE IN THE FIRST PLACE?
            // THIS IS HOW ITS CONTENTS LOOK AFTER DECOMPRESSION
            // blob <size>\0<content>

            // Check if the user has provided the blob sha
            if args.len() < 3 {
                eprintln!("Usage: {} cat-file -p <blob_sha>", args[0]);
                return Ok(());
            }

            let blob_sha = &args[3];
            cat_file(blob_sha)?;
        }
        "hash-object" => {
            // First I need to read the contents of the file
            // store = header + content
            // header seems to be "blob <size>\0"
            // size = length of content
            // content = actual file content which is given input as test.txt
            // then hash the stored content

            // Check if the user has provided the file name
            if args.len() < 3 {
                eprintln!("Usage: {} hash-object -w <file>", args[0]);
                return Ok(());
            }

            // Read the file content
            let file_name = &args[3];
            let content = fs::read(file_name)?;

            let header = format!("blob {}\0", content.len());
            let blob_data = format!("{}{}", header, String::from_utf8_lossy(&content));
            // print!("{}", blob_data);

            // Compute the SHA-1 hash of the blob_data
            let mut hasher = Sha1::new();
            hasher.update(blob_data.as_bytes());
            let hash = hasher.finalize();
            let hash_hex = format!("{:x}", hash);
            print!("{}", hash_hex);

            // Create the directory structure in .git/objects
            let (dir, file) = hash_hex.split_at(2);
            let object_dir = format!(".git/objects/{}", dir);
            fs::create_dir_all(&object_dir)?;

            // Compress the blob data
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(blob_data.as_bytes())?;
            let compressed_data = encoder.finish()?;

            // Write the compressed data to the object file
            let object_path = format!("{}/{}", object_dir, file);
            fs::write(object_path, compressed_data)?;
        }
        "ls-tree" => {
            // Check if the user has provided the tree sha
            if args.len() < 3 {
                eprintln!("Usage: {} ls-tree --name-only <tree_sha>", args[0]);
                return Ok(());
            }

            let tree_sha = &args[3];
            let (dir, file) = tree_sha.split_at(2);
            let path = format!(".git/objects/{}/{}", dir, file);
            let content = fs::read(path)?;

            // Decompress the data
            let mut decompressed_data = ZlibDecoder::new(&content[..]);
            let mut tree_file_contents_vec = Vec::new();
            decompressed_data.read_to_end(&mut tree_file_contents_vec)?;

            // Convert the tree file contents to a readable string
            let readable_tree = String::from_utf8_lossy(&tree_file_contents_vec);

            let cow_str: Cow<str> = Cow::Borrowed(&readable_tree);

            // Extract the tree entries
            let names = extract_names_from_tree_entries(cow_str.as_bytes());
            for name in names {
                println!("{}", name);
            }
        }
        "write-tree" => {
            // Check if the user has provided the tree sha
            if args.len() < 2 {
                eprintln!("Usage: {} write-tree", args[0]);
                return Ok(());
            }

            let tree_sha = write_tree(Path::new("."))?;
            print!("{}", tree_sha.to_hex());
        }
        "commit-tree" => {
            // Check if the user has provided the tree sha
            if args.len() < 3 {
                eprintln!(
                    "Usage: {} commit-tree <tree_sha> -p <commit_sha> -m <message>",
                    args[0]
                );
                return Ok(());
            }

            let tree_sha = &args[2];
            let parent_sha = Some(&args[4]);
            let message = &args[6];
            let author = "Rohit Paul <Rohit.paul@gmail.com>";
            let committer = "Kishor Kumar Paroi <kishor.ruet.cse@gmail.com>";

            let commit_data =
                create_commit_object(tree_sha, parent_sha, author, committer, message);
            let commit_hash = write_commit_object(&commit_data)?;

            println!("{}", commit_hash.to_hex());
        }
        "clone" => {
            // Check if the user has provided the repository URL
            if args.len() < 3 {
                eprintln!("Usage: {} clone <repository_url> <directory>", args[0]);
                return Ok(());
            }

            let repository_url = &args[2];
            let directory = &args[3];

            clone_repository(repository_url, directory)?;
        }

        _ => {
            eprintln!("Unknown command: {}", args[1]);
        }
    }

    Ok(())
}

fn clone_repository(repository_url: &str, directory: &str) -> io::Result<()> {
    // Step 1: Create the local directory if it doesn't exist
    if !Path::new(directory).exists() {
        fs::create_dir_all(directory)?;
    }

    // Step 2: Initialize the directory as git repository
    let output = Command::new("git").arg("init").arg(directory).output()?;

    if !output.status.success() {
        eprintln!(
            "Failed to initialize the directory as git repository : {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to initialize the directory as git repository",
        ));
    }

    // Step 3: Add the remote repository as origin
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("remote")
        .arg("add")
        .arg("origin")
        .arg(repository_url)
        .output()?;

    if !output.status.success() {
        eprintln!(
            "Failed to add the remote repository as origin : {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to add the remote repository as origin",
        ));
    }

    // Step 4: Fetch the objects from the remote repository
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("fetch")
        .arg("origin")
        .output()?;
    if !output.status.success() {
        eprintln!(
            "Failed to fetch the objects from the remote repository : {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to fetch the objects from the remote repository",
        ));
    }

    // Step 5: Checkout the master branch
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .arg("checkout")
        .arg("origin/master")
        .output()?;

    if !output.status.success() {
        eprintln!(
            "Failed to checkout the master branch : {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to checkout the master branch",
        ));
    }

    println!("Cloned repository from {} to {}", repository_url, directory);
    Ok(())
}

fn write_commit_object(commit_data: &str) -> io::Result<Hash> {
    let commit_bytes = commit_data.as_bytes();
    let header = format!("commit {}\0", commit_bytes.len());
    let full_commit_data = [header.as_bytes(), commit_bytes].concat();

    let hash = compute_sha1(&full_commit_data);
    write_object(&hash, &full_commit_data)?;
    Ok(hash)
}

fn get_current_time() -> String {
    let now: DateTime<Utc> = Utc::now();
    now.format("%a %b %e %H:%M:%S %Y %z").to_string()
}

fn create_commit_object(
    tree_sha: &str,
    parent_sha: Option<&String>,
    author: &str,
    committer: &str,
    message: &str,
) -> String {
    let mut commit_data = format!("tree {}\n", tree_sha);

    if let Some(parent) = parent_sha {
        commit_data.push_str(&format!("parent {}\n", parent));
    }

    commit_data.push_str(&format!("author {} {}\n", author, get_current_time()));
    commit_data.push_str(&format!("committer {} {}\n", committer, get_current_time()));
    commit_data.push_str(&format!("\n{}\n", message));

    commit_data
}

fn write_tree(path: &Path) -> io::Result<Hash> {
    let mut entries = Vec::new();

    // Iterate over the files/directories in the working directory
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().into_string().unwrap();

        if path.is_file() {
            let hash = create_blob(&path)?;
            entries.push(GitTreeEntry {
                mode: "100644".to_string(),
                name: file_name,
                hash,
            });
        } else if path.is_dir() && file_name != ".git" {
            let hash = write_tree(&path)?;
            entries.push(GitTreeEntry {
                mode: "40000".to_string(),
                name: file_name,
                hash,
            });
        }
    }

    // Sort the entries by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    // Create the tree object
    let mut tree_data = Vec::new();
    for entry in entries {
        let mode = entry.mode;
        let name = entry.name;
        let hash = entry.hash.to_hex();
        tree_data.extend_from_slice(mode.as_bytes());
        tree_data.push(b' ');
        tree_data.extend_from_slice(name.as_bytes());
        tree_data.push(0);
        tree_data.extend_from_slice(&hex::decode(hash).unwrap());
    }

    // Add the tree header
    let header = format!("tree {}\0", tree_data.len());
    let mut result = Vec::from(header.as_bytes());
    result.extend_from_slice(&tree_data);

    // Compute the SHA-1 hash of the tree_data
    let hash = compute_sha1(&result);
    write_object(&hash, &result)?;
    Ok(hash)
}

fn compute_sha1(data: &[u8]) -> Hash {
    let mut hasher = Sha1::new();
    hasher.update(data);
    Hash::from_bytes(&hasher.finalize()).unwrap()
}

fn create_blob(path: &Path) -> io::Result<Hash> {
    // Read the file content
    let mut file = fs::File::open(path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    // Create the blob data
    let header = format!("blob {}", contents.len());
    let mut result = Vec::from(header.as_bytes());
    result.push(0); // null byte
    result.extend_from_slice(&contents);

    // Compute the SHA-1 hash of the blob_data
    let hash = compute_sha1(&result);
    write_object(&hash, &result)?;
    Ok(hash)
}

fn write_object(hash: &Hash, data: &[u8]) -> io::Result<()> {
    let hash_hex = hash.to_hex();
    let (dir, file) = hash_hex.split_at(2);
    let object_dir = format!(".git/objects/{}", dir);
    fs::create_dir_all(&object_dir)?;

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    let compressed_data = encoder.finish()?;
    let object_path = format!("{}/{}", object_dir, file);
    fs::write(object_path, compressed_data)?;
    Ok(())
}

fn extract_names_from_tree_entries(tree_object: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut i = 0;

    //tree <size>\0<mode> <name>\0<20_byte_sha><mode> <name>\0<20_byte_sha>

    // Skip the tree header
    while i < tree_object.len() {
        if let Some(null_pos) = tree_object[i..].iter().position(|&b| b == b'\0') {
            i = null_pos + 1;
            break;
        }
        i += 1;
    }

    // <mode> <name>\0<20_byte_sha><mode> <name>\0<20_byte_sha>

    while i < tree_object.len() {
        // Find the null byte that separates the mode and the name
        if let Some(null_pos) = tree_object[i..].iter().position(|&b| b == b'\0') {
            // Extract the mode and the name parts
            // <mode> <name>
            let entry = &tree_object[i..i + null_pos];
            if let Some(space_pos) = entry.iter().position(|&b| b == b' ') {
                // Extract the name part
                // <name>
                let name = &entry[space_pos + 1..];
                if let Ok(name) = std::str::from_utf8(name) {
                    // check if the entry is a tree or a blob
                    names.push(name.to_string());
                }
            }
            // Move to the next entry in the tree object
            i += null_pos + 20 + 1; // 20 bytes for the SHA-1 hash + 1 for the null byte
        } else {
            break;
        }
    }
    names
}

fn cat_file(blob_sha: &str) -> io::Result<()> {
    // Step 1: identify the file & read from it
    //object directory is in form of .git/objects/[first 2 hash digits]/[remaining hash digits after that]
    //ex - .git/objects/e8/8f7a929cd70b0274c4ea33b209c97fa845fdbc

    let (dir, file) = blob_sha.split_at(2);
    let path = format!(".git/objects/{}/{}", dir, file);
    let content = fs::read(path)?;

    // Step 2: Decompress the data
    let mut decompressed_data = ZlibDecoder::new(&content[..]);
    // Data would be something like this: "x\x9CK\xCA\xC9OR04c(\xCFH,Q\xC8,V(-\xD0QH\xC9O\xB6\a\x00_\x1C\a\x9D"

    //Step3: EXTRACT CONTENT from the DECOMPRESSED DATA
    let mut blob_file_contents_vec = Vec::new();

    //Step4: Filling the buffer with contents of blob file
    decompressed_data.read_to_end(&mut blob_file_contents_vec)?;

    //Step5: Convert the blob file contents to a readable string
    let readable_blob = String::from_utf8_lossy(&blob_file_contents_vec);

    //Step6: Now extract <content> from blob <size>\0<content>
    match extract_content(&readable_blob) {
        Some(content) => print!("{}", content.to_string().trim_end()),
        None => println!("Invalid blob object file"),
    }

    Ok(())
}

fn extract_content(blob_file_contents: &str) -> Option<&str> {
    // Find the position of the first null byte

    if let Some(pos) = blob_file_contents.find('\0') {
        // Extract the content after the null byte
        Some(&blob_file_contents[pos + 1..])
    } else {
        None
    }
}
