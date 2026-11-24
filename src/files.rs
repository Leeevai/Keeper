//! Lessons made of your own code: lines of the source files in a file or
//! folder you choose.

use std::fs;
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

const MAX_FILES: usize = 2_000;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: u64 = 4 * 1024 * 1024;
/// Longer lines are usually generated or minified.
const MAX_LINE_CHARS: usize = 120;

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

/// The lines of a file, or of the source files in a folder and its
/// subfolders, that can be typed. Indentation and runs of spaces are
/// removed, and lines with a character that `typeable` rejects are left
/// out, as are binary files and very large ones.
pub fn read_lines(path: &Path, typeable: impl Fn(char) -> bool) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_file() {
        files.push(path.to_path_buf());
    } else {
        collect(path, &mut files);
    }
    let mut lines = Vec::new();
    let mut total = 0;
    for file in files {
        let Ok(meta) = fs::metadata(&file) else {
            continue;
        };
        if meta.len() > MAX_FILE_BYTES || total + meta.len() > MAX_TOTAL_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&file) else {
            continue;
        };
        if bytes.contains(&0) {
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        total += meta.len();
        lines.extend(text.lines().filter_map(|l| clean(l, &typeable)));
    }
    lines
}

/// Adds the source files under a folder, in name order.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if files.len() >= MAX_FILES {
            return;
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
            collect(&path, files);
        } else if meta.is_file() && is_source(&path) {
            files.push(path);
        }
    }
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

/// Consecutive lines from a random place, joined with spaces, until the
/// text is at least `length` characters long, as other lessons are. Lessons
/// start at the start of a line, and wrap around from the last line to the
/// first, without repeating one.
pub fn fragment(lines: &[String], length: usize, rng: &mut dyn Rng) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let start = (rng.next_f64() * lines.len() as f64) as usize % lines.len();
    let mut text = String::new();
    let mut chars = 0;
    for line in lines.iter().cycle().skip(start).take(lines.len()) {
        let n = line.chars().count();
        if chars > 0 {
            text.push(' ');
            chars += 1;
        }
        text.push_str(line);
        chars += n;
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

    #[test]
    fn reads_the_source_files_of_a_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let write = |path: &str, text: &[u8]| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("src/main.rs", b"fn main() {\n\tprintln!(\"hi\");\n}\n");
        write("src/lib.py", b"def  f(x):\n    return x  +  1\n\n");
        write("notes.unknown", b"not read: unknown type\n");
        write("node_modules/dep/index.js", b"not read: dependency\n");
        write("target/debug/out.rs", b"not read: build output\n");
        write(".git/config.toml", b"not read: hidden\n");
        write("src/binary.rs", b"not\0read\n");
        write("src/emoji.md", "smile \u{1F600}\nplain line\n".as_bytes());
        write(
            "src/long.js",
            format!("{}\nshort\n", "x".repeat(200)).as_bytes(),
        );
        let lines = read_lines(root, ascii);
        assert_eq!(
            lines,
            vec![
                "plain line",
                "def f(x):",
                "return x + 1",
                "short",
                "fn main() {",
                "println!(\"hi\");",
                "}",
            ]
        );
        // A file chosen on its own is read whatever its type.
        assert_eq!(
            read_lines(&root.join("notes.unknown"), ascii),
            vec!["not read: unknown type"]
        );
        assert!(read_lines(&root.join("missing"), ascii).is_empty());
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
    fn lessons_are_whole_consecutive_lines() {
        let lines: Vec<String> = (0..50).map(|i| format!("line {i:02} of code")).collect();
        let mut rng = Lcg::new(3);
        for _ in 0..20 {
            let text = fragment(&lines, 100, &mut rng);
            let n = text.chars().count();
            assert!((100..=117).contains(&n), "{n}: {text}");
            let parts: Vec<&str> = text.split(" line ").collect();
            assert!(text.starts_with("line "), "{text}");
            // Consecutive, wrapping from line 49 to line 0.
            let numbers: Vec<usize> = std::iter::once(&text[5..7])
                .chain(parts[1..].iter().map(|p| &p[..2]))
                .map(|d| d.parse().unwrap())
                .collect();
            for pair in numbers.windows(2) {
                assert_eq!(pair[1], (pair[0] + 1) % 50, "{text}");
            }
        }
        // A short file repeats no line, and a long line is kept whole.
        assert_eq!(fragment(&["a".into(), "b".into()], 100, &mut rng).len(), 3);
        let long = vec!["x".repeat(110)];
        assert_eq!(fragment(&long, 100, &mut rng), "x".repeat(110));
        assert_eq!(fragment(&[], 100, &mut rng), "");
    }
}
