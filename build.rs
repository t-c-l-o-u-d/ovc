// GNU Affero General Public License v3.0 or later (see LICENSE or https://www.gnu.org/licenses/agpl.txt)

use std::fs;
use std::path::Path;

use clap::CommandFactory;

include!("src/cli.rs");

fn main() {
    println!("cargo::rerun-if-changed=src/cli.rs");

    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd);

    let mut buffer: Vec<u8> = Vec::new();
    man.render(&mut buffer).expect("failed to render man page");

    // Write to OUT_DIR so the binary can embed the man page at compile time
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    fs::write(Path::new(&out_dir).join("ovc.1"), &buffer).expect("failed to write man page");

    // Also write to the source tree for git commits.
    // Silently skip if the filesystem is read-only (e.g. container linters).
    let man_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("man");
    if fs::create_dir_all(&man_dir).is_ok() {
        let _ = fs::write(man_dir.join("ovc.1"), buffer);
    }
}
