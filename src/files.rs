//! Lessons made of your own code: lines of the source files in a file or
//! folder you choose.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::rng::Rng;

/// The file types read from a folder. A file chosen on its own is read
/// whatever its type.
pub const EXTENSIONS: &[&str] = &[
    "c", "cc", "cpp", "cs", "css", "dart", "ex", "exs", "go", "h", "hpp", "hs", "html", "java",
    "js", "jsx", "kt", "kts", "lua", "md", "php", "py", "rb", "rs", "scala", "scss", "sh", "sql",
    "svelte", "swift", "toml", "ts", "tsx", "txt", "vue", "yaml", "yml", "zig",
];

/// Folders of dependencies and build output, which are not your code.
/// Hidden folders, such as `.git`, are skipped too.
const SKIPPED_FOLDERS: &[&str] = &[
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
    "vendor",
    "venv",
];

/// Longer lines are usually generated or minified.
const MAX_LINE_CHARS: usize = 120;

/// How much of a folder is looked at, so that a huge folder cannot stall
/// the start of a lesson.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Folder entries looked at, files and folders alike.
    pub entries: usize,
    /// Source files read.
    pub files: usize,
    /// The largest file read.
    pub file_bytes: u64,
    /// All the bytes read, kept or not.
    pub total_bytes: u64,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        entries: 50_000,
        files: 2_000,
        file_bytes: 256 * 1024,
        total_bytes: 4 * 1024 * 1024,
    };
}

/// Expands a leading `~` to the home directory.
pub fn expand_home(path: &str) -> PathBuf {
    let path = path.trim();
    match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => match dirs::home_dir() {
            Some(home) => home.join(rest.trim_start_matches('/')),
            None => PathBuf::from(path),
        },
        _ => PathBuf::from(path),
    }
}

/// A path as it can be shown in the terminal: control characters, which
/// the terminal would act on, are replaced.
pub fn printable(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_control() { '\u{FFFD}' } else { c })
        .collect()
}

/// The lines of a file, or of each source file in a folder and its
/// subfolders, that can be typed, file by file. Indentation and runs of
/// spaces are removed, and lines with a character that `typeable` rejects
/// are left out, as are binary files and very large ones. Fails only when
/// the path itself cannot be read; files inside a folder that cannot be
/// read are skipped.
pub fn read(path: &Path, typeable: impl Fn(char) -> bool) -> io::Result<Vec<Vec<String>>> {
    read_with(path, typeable, Limits::DEFAULT)
}

pub fn read_with(
    path: &Path,
    typeable: impl Fn(char) -> bool,
    limits: Limits,
) -> io::Result<Vec<Vec<String>>> {
    let meta = fs::metadata(path)?;
    let single = !meta.is_dir();
    let mut files = Vec::new();
    if single {
        files.push(path.to_path_buf());
    } else {
        let mut entries = limits.entries;
        collect(path, &mut files, &mut entries, &limits, true)?;
    }
    let mut lines = Vec::new();
    let mut total = 0;
    for file in files {
        if total >= limits.total_bytes {
            break;
        }
        let bytes = match read_file(&file, limits.file_bytes) {
            Ok(bytes) => bytes,
            Err(e) if single => return Err(e),
            Err(_) => continue,
        };
        // Every byte read counts, also those of a file left out below.
        total += bytes.len() as u64;
        if bytes.len() as u64 > limits.file_bytes || total > limits.total_bytes {
            continue;
        }
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let kept: Vec<String> = text.lines().filter_map(|l| clean(l, &typeable)).collect();
        if !kept.is_empty() {
            lines.push(kept);
        }
    }
    Ok(lines)
}

/// Reads at most one byte more than `limit`, which tells a file that is too
/// large, even one that grew after it was listed.
fn read_file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Adds the source files under a folder, in name order, while entries and
/// files are left in the limits.
fn collect(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    entries: &mut usize,
    limits: &Limits,
    root: bool,
) -> io::Result<()> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if root => return Err(e),
        Err(_) => return Ok(()),
    };
    // Never more entries than the limit, however large the folder.
    let mut paths: Vec<PathBuf> = listing.flatten().take(*entries).map(|e| e.path()).collect();
    *entries -= paths.len();
    paths.sort();
    for path in paths {
        if files.len() >= limits.files {
            break;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(".");
        if name.starts_with('.') || SKIPPED_FOLDERS.contains(&name) {
            continue;
        }
        // Links are not followed, so that a link cannot make a loop.
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            collect(&path, files, entries, limits, false)?;
        } else if meta.is_file() && is_source(&path) && meta.len() <= limits.file_bytes {
            files.push(path);
        }
    }
    Ok(())
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn clean(line: &str, typeable: impl Fn(char) -> bool) -> Option<String> {
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let length = line.chars().count();
    let ok =
        length > 0 && length <= MAX_LINE_CHARS && line.chars().all(|c| c == ' ' || typeable(c));
    ok.then_some(line)
}

/// Consecutive lines of one file from a random place, joined with spaces,
/// until the text is at least `length` characters long, as other lessons
/// are. Every line is as likely to start a lesson. A lesson starts at the
/// start of a line and wraps around within its file, without repeating a
/// line, so a short file makes a shorter lesson.
pub fn fragment(files: &[Vec<String>], length: usize, rng: &mut dyn Rng) -> String {
    let count: usize = files.iter().map(Vec::len).sum();
    if count == 0 {
        return String::new();
    }
    let mut start = (rng.next_f64() * count as f64) as usize % count;
    let mut lines: &[String] = &[];
    for file in files {
        if start < file.len() {
            lines = file;
            break;
        }
        start -= file.len();
    }
    let mut text = String::new();
    let mut chars = 0;
    for line in lines.iter().cycle().skip(start).take(lines.len()) {
        if chars > 0 {
            text.push(' ');
            chars += 1;
        }
        text.push_str(line);
        chars += line.chars().count();
        if chars >= length {
            break;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Lcg;

    fn ascii(c: char) -> bool {
        c.is_ascii_graphic()
    }

    fn write(root: &Path, path: &str, text: &[u8]) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn reads_the_source_files_of_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "src/main.rs",
            b"fn main() {\n\tprintln!(\"hi\");\n}\n",
        );
        write(root, "src/lib.py", b"def  f(x):\n    return x  +  1\n\n");
        write(root, "notes.unknown", b"not read: unknown type\n");
        write(root, "node_modules/dep/index.js", b"not read: dependency\n");
        write(root, "target/debug/out.rs", b"not read: build output\n");
        write(root, ".git/config.toml", b"not read: hidden\n");
        write(root, "src/binary.rs", b"not\0read\n");
        write(
            root,
            "src/emoji.md",
            "smile \u{1F600}\nplain line\n".as_bytes(),
        );
        write(
            root,
            "src/long.js",
            format!("{}\nshort\n", "x".repeat(200)).as_bytes(),
        );
        let files = read(root, ascii).unwrap();
        assert_eq!(
            files,
            vec![
                vec!["plain line"],
                vec!["def f(x):", "return x + 1"],
                vec!["short"],
                vec!["fn main() {", "println!(\"hi\");", "}"],
            ]
        );
        // A file chosen on its own is read whatever its type.
        assert_eq!(
            read(&root.join("notes.unknown"), ascii).unwrap(),
            vec![vec!["not read: unknown type"]]
        );
        // A folder without code is not an error, a missing path is.
        fs::create_dir(root.join("empty")).unwrap();
        assert!(read(&root.join("empty"), ascii).unwrap().is_empty());
        let missing = read(&root.join("missing"), ascii).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn large_folders_and_files_stay_within_the_limits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for i in 0..20 {
            let text = format!("let file{i:02} = 0;\n");
            write(root, &format!("f{i:02}.rs"), text.as_bytes());
        }
        let small = Limits {
            entries: 5,
            files: 100,
            file_bytes: 1000,
            total_bytes: 1_000_000,
        };
        // Only five entries are looked at, whichever the folder lists first.
        assert_eq!(read_with(root, ascii, small).unwrap().len(), 5);
        let few_files = Limits {
            entries: 100,
            files: 3,
            ..small
        };
        assert_eq!(read_with(root, ascii, few_files).unwrap().len(), 3);
        // Binary files that are left out still use up the bytes to read.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.rs", &[0u8; 600]);
        write(root, "b.rs", b"let kept = true;\n");
        write(root, "c.rs", b"let late = true;\n");
        let budget = Limits {
            entries: 100,
            files: 100,
            file_bytes: 1000,
            total_bytes: 620,
        };
        assert_eq!(
            read_with(root, ascii, budget).unwrap(),
            vec![vec!["let kept = true;"]]
        );
        // A file over the size limit is left out, even when chosen alone.
        let tiny = Limits {
            file_bytes: 10,
            ..budget
        };
        assert!(
            read_with(&root.join("b.rs"), ascii, tiny)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn expands_the_home_directory() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_home("~/code"), home.join("code"));
        assert_eq!(expand_home(" ~ "), home);
        assert_eq!(expand_home("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand_home("~other/x"), PathBuf::from("~other/x"));
    }

    #[test]
    fn control_characters_are_not_shown() {
        assert_eq!(
            printable("/code/\x1b[2Jproject\n"),
            "/code/\u{FFFD}[2Jproject\u{FFFD}"
        );
        assert_eq!(printable("~/código"), "~/código");
    }

    #[test]
    fn lessons_are_whole_consecutive_lines_of_one_file() {
        let file = |name: &str, n: usize| -> Vec<String> {
            (0..n)
                .map(|i| format!("{name} line {i:02} of code"))
                .collect()
        };
        let files = vec![file("first", 50), file("second", 50)];
        let mut rng = Lcg::new(3);
        let mut seen = [false, false];
        for _ in 0..40 {
            let text = fragment(&files, 100, &mut rng);
            let n = text.chars().count();
            assert!((100..=130).contains(&n), "{n}: {text}");
            // Whole lines of one file, from the first line on, wrapping from
            // its last line to its first, joined by single spaces.
            let index = usize::from(text.starts_with("second"));
            seen[index] = true;
            let lines = &files[index];
            let start = lines
                .iter()
                .position(|l| text.starts_with(l.as_str()))
                .unwrap();
            let count = text.matches(" line ").count();
            let expected: Vec<&str> = (0..count)
                .map(|k| lines[(start + k) % lines.len()].as_str())
                .collect();
            assert_eq!(text, expected.join(" "));
        }
        assert_eq!(seen, [true, true], "lessons come from both files");
        // A short file is used whole, once, and a long line is kept whole.
        let short = vec![vec!["a".to_string(), "b".to_string()]];
        assert_eq!(fragment(&short, 100, &mut rng), "a b");
        let long = vec![vec!["x".repeat(110)]];
        assert_eq!(fragment(&long, 100, &mut rng), "x".repeat(110));
        assert_eq!(fragment(&[], 100, &mut rng), "");
        assert_eq!(fragment(&[vec![]], 100, &mut rng), "");
    }
}
