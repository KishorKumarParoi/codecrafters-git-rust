use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha1::{Digest, Sha1};
use std::borrow::Cow;
use std::env;
use std::fs;
use std::io::prelude::*;
use std::io::{self, Read};

/*The output of git write-tree is the 40-char
SHA hash of the tree object that was written to .git/objects.

To implement this, you'll need to:

Iterate over the files/directories in the working directory

If the entry is a file, create a blob object and record its SHA hash

If the entry is a directory, recursively create a tree
object and record its SHA hash

Once you have all the entries and their SHA hashes, write
 the tree object to the .git/objects directory

If you're testing this against git locally, make sure to
 run git add . before git write-tree, so that all files in the working directory are staged.
*/

//[CONTINUATION PROJECT] - IMPLEMENTATING GIT FROM SCRATCH

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

            let tree_sha = write_tree(".")?;
            print!("{}", tree_sha);
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
        }
    }

    Ok(())
}

fn write_tree(dir: &str) -> io::Result<String> {
    let mut entries = Vec::new();

    // Iterate over the files/directories in the working directory

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let metadata = fs::metadata(&path)?;

        // Exclude the .git directory
        if path.file_name().unwrap() == ".git" {
            continue;
        }

        if metadata.is_file() {
            let blob_sha = create_blob(&path)?;
            let mode = "100644"; // regular file
            let name = path.file_name().unwrap().to_str().unwrap();
            entries.push(format!(
                "{} {}\0{}",
                mode,
                name,
                hex_to_string(&hex_to_bytes(&blob_sha))
            ));
        } else if metadata.is_dir() {
            let tree_sha = write_tree(&path.to_str().unwrap())?;
            let mode = "040000"; // directory
            let name = path.file_name().unwrap().to_str().unwrap();
            entries.push(format!(
                "{} {}\0{}",
                mode,
                name,
                hex_to_string(&hex_to_bytes(&tree_sha))
            ));
        }
    }

    // Sort the entries by name
    entries.sort_by(|a, b| {
        let a_name = a
            .split('\0')
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        let b_name = b
            .split('\0')
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        a_name.cmp(b_name)
    });

    // Create Tree object, Sort the entries and concatenate them
    let tree_data = entries.join("");
    let header = format!("tree {}\0", tree_data.len());
    let tree_object = format!("{}{}", header, tree_data);

    // Compute the SHA-1 hash of the tree_object
    let mut hasher = Sha1::new();
    hasher.update(tree_object.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);

    // Create the directory structure in .git/objects
    let (dir, file) = hash_hex.split_at(2);
    let object_dir = format!(".git/objects/{}", dir);
    fs::create_dir_all(&object_dir)?;

    // Compress the tree data
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(tree_object.as_bytes())?;
    let compressed_data = encoder.finish()?;

    // Write the compressed data to the object file
    let object_path = format!("{}/{}", object_dir, file);
    fs::write(object_path, compressed_data)?;

    Ok(hash_hex)
}

fn create_blob(path: &std::path::Path) -> io::Result<String> {
    // Read the file content
    let mut file = fs::File::open(path)?;
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;

    // Create the blob data
    let header = format!("blob {}\0", contents.len());
    let blob_data = format!("{}{}", header, String::from_utf8_lossy(&contents));

    // Compute the SHA-1 hash of the blob_data
    let mut hasher = Sha1::new();
    hasher.update(blob_data.as_bytes());
    let hash = hasher.finalize();
    let hash_hex = format!("{:x}", hash);

    // Create the directory structure in .git/objects
    let (dir, file) = hash_hex.split_at(2);
    let object_dir = format!(".git/objects/{}", dir);
    fs::create_dir_all(&object_dir)?;

    // Compress the blob data
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(blob_data.as_bytes())?;
    let compressed_data = encoder.finish()?;
    let object_path = format!("{}/{}", object_dir, file);
    fs::write(object_path, compressed_data)?;

    Ok(hash_hex)
}

fn hex_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let high = chunk[0] as char;
            let low = chunk[1] as char;
            let high = high.to_digit(16).unwrap() as u8;
            let low = low.to_digit(16).unwrap() as u8;
            (high << 4) | low
        })
        .collect()
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

